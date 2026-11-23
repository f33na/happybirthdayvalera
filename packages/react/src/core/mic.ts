import { behavior } from './data';
import type { ErrorCode } from './machine';

export interface MicBlock {
  /** Mono Float32 PCM at 16 kHz, about 20 ms. */
  pcm: Float32Array;
  rms: number;
  /** Block length in ms. */
  ms: number;
}

export interface MicHandle {
  stop(): void;
}

export class MicError extends Error {
  readonly code: ErrorCode;
  constructor(code: ErrorCode, message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = 'MicError';
    this.code = code;
  }
}

export const WORKLET_NAME = 'hbv-capture';

/**
 * Body of the AudioWorklet module. It runs in the worklet global scope, so it must not touch anything
 * outside itself: it is turned into a string with `Function.prototype.toString`. It resamples the input
 * to `targetRate` (box filter when downsampling, then linear interpolation) and posts blocks of
 * `blockSize` samples with their RMS.
 */
export function workletMain(name: string, targetRate: number, blockSize: number): void {
  const scope = globalThis as unknown as {
    sampleRate: number;
    registerProcessor(name: string, ctor: unknown): void;
    AudioWorkletProcessor: new () => {
      port: { postMessage(m: unknown, t?: unknown[]): void; onmessage: ((e: { data: unknown }) => void) | null };
    };
  };

  // State lives in closure variables, not class fields: a lowering bundler would hoist field helpers
  // out of this function, and the function is serialized on its own.
  const ratio = scope.sampleRate / targetRate;
  let acc = 0;
  let phase = 0;
  let n = 0;
  let block = new Float32Array(blockSize);
  let filled = 0;
  let active = true;

  class CaptureProcessor extends scope.AudioWorkletProcessor {
    constructor() {
      super();
      this.port.onmessage = (e) => {
        if (e.data === 'stop') active = false;
      };
    }

    process(inputs: Float32Array[][]): boolean {
      const channel = inputs[0]?.[0];
      if (!channel) return active;
      // Each output sample is the mean of `ratio` input samples; a fractional ratio carries over.
      for (let i = 0; i < channel.length; i++) {
        acc += channel[i] ?? 0;
        n += 1;
        phase += 1;
        if (phase >= ratio) {
          block[filled++] = acc / n;
          acc = 0;
          n = 0;
          // Keep the fraction so a ratio like 2.75 still averages out to exactly 16 kHz.
          phase -= ratio;
          if (filled === blockSize) {
            let sum = 0;
            for (let k = 0; k < blockSize; k++) sum += (block[k] ?? 0) ** 2;
            const pcm = block;
            this.port.postMessage({ pcm, rms: Math.sqrt(sum / blockSize) }, [pcm.buffer]);
            block = new Float32Array(blockSize);
            filled = 0;
          }
        }
      }
      return active;
    }
  }
  scope.registerProcessor(name, CaptureProcessor);
}

export function workletSource(
  name = WORKLET_NAME,
  targetRate: number = behavior.sampleRate,
  blockSize = Math.round(targetRate * 0.02),
): string {
  return `(${workletMain.toString()})(${JSON.stringify(name)}, ${targetRate}, ${blockSize});`;
}

function mapGetUserMediaError(error: unknown): MicError {
  const name = error instanceof DOMException ? error.name : '';
  if (name === 'NotAllowedError' || name === 'SecurityError') {
    return new MicError('mic-denied', 'Microphone access denied', { cause: error });
  }
  return new MicError('mic-unavailable', 'No usable microphone', { cause: error });
}

/**
 * Opens the microphone and streams ~20 ms blocks at 16 kHz mono. Rejects with `MicError`.
 * `isCancelled` lets the caller drop the stream when STOP arrives while the permission prompt is open.
 */
export async function openMic(onBlock: (block: MicBlock) => void, isCancelled: () => boolean): Promise<MicHandle> {
  if (typeof navigator === 'undefined' || !navigator.mediaDevices?.getUserMedia || typeof AudioContext === 'undefined') {
    throw new MicError('unsupported', 'getUserMedia or AudioContext is not available');
  }
  let stream: MediaStream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({
      audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true },
    });
  } catch (error) {
    throw mapGetUserMediaError(error);
  }

  let context: AudioContext | null = null;
  let url: string | null = null;
  const release = (): void => {
    for (const track of stream.getTracks()) track.stop();
    if (url) URL.revokeObjectURL(url);
    void context?.close().catch(() => undefined);
  };

  try {
    if (isCancelled()) throw new MicError('mic-unavailable', 'Cancelled');
    context = new AudioContext();
    if (!context.audioWorklet) throw new MicError('unsupported', 'AudioWorklet is not available');
    // Inline Blob module: consumers' bundlers need no extra asset for the worklet.
    url = URL.createObjectURL(new Blob([workletSource()], { type: 'text/javascript' }));
    await context.audioWorklet.addModule(url);
    const node = new AudioWorkletNode(context, WORKLET_NAME, { numberOfInputs: 1, numberOfOutputs: 0 });
    const blockMs = (Math.round(behavior.sampleRate * 0.02) / behavior.sampleRate) * 1000;
    node.port.onmessage = (e: MessageEvent<{ pcm: Float32Array; rms: number }>) => {
      onBlock({ pcm: e.data.pcm, rms: e.data.rms, ms: blockMs });
    };
    context.createMediaStreamSource(stream).connect(node);
    if (context.state === 'suspended') await context.resume();
  } catch (error) {
    release();
    throw error instanceof MicError ? error : new MicError('mic-unavailable', 'Audio capture failed', { cause: error });
  }

  return { stop: release };
}
