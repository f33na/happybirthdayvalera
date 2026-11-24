import { afterEach, describe, expect, it, vi } from 'vitest';
import { workletSource } from '../core/mic';

interface Out {
  pcm: Float32Array;
  rms: number;
}

/** Runs the serialized worklet in a fake AudioWorkletGlobalScope and feeds it 128-frame render quanta. */
function runWorklet(inputRate: number, input: Float32Array): Out[] {
  const out: Out[] = [];
  let Processor: (new () => { process(i: Float32Array[][]): boolean }) | null = null;
  class AudioWorkletProcessor {
    port = { postMessage: (m: Out) => out.push(m), onmessage: null as unknown };
  }
  vi.stubGlobal('sampleRate', inputRate);
  vi.stubGlobal('AudioWorkletProcessor', AudioWorkletProcessor);
  vi.stubGlobal('registerProcessor', (_name: string, ctor: typeof Processor) => (Processor = ctor));
  // The worklet source is a string by design (it ships as a Blob URL), so a Function is the only way to run it here.
  new Function(workletSource())();
  if (!Processor) throw new Error('processor was not registered');
  const processor = new (Processor as new () => { process(i: Float32Array[][]): boolean })();
  for (let i = 0; i < input.length; i += 128) {
    processor.process([[input.subarray(i, Math.min(i + 128, input.length))]]);
  }
  return out;
}

const sine = (rate: number, seconds: number, amp: number): Float32Array =>
  Float32Array.from({ length: Math.round(rate * seconds) }, (_, i) => amp * Math.sin((2 * Math.PI * 440 * i) / rate));

describe('capture worklet', () => {
  afterEach(() => vi.unstubAllGlobals());

  it.each([48000, 44100, 16000])('resamples %i Hz to 20 ms blocks at 16 kHz', (rate) => {
    const out = runWorklet(rate, sine(rate, 1, 0.5));
    // 1 s of audio is 50 blocks of 320 samples; the resampler may hold back the last partial block.
    expect(out.length).toBeGreaterThanOrEqual(48);
    expect(out.length).toBeLessThanOrEqual(50);
    for (const block of out) expect(block.pcm).toHaveLength(320);
  });

  it('reports the RMS of the block (a sine of amplitude a has RMS a/sqrt(2))', () => {
    const out = runWorklet(48000, sine(48000, 0.5, 0.5));
    const mid = out[10];
    expect(mid?.rms).toBeGreaterThan(0.3);
    expect(mid?.rms).toBeLessThan(0.37);
  });

  it('reports silence as zero', () => {
    const out = runWorklet(48000, new Float32Array(48000));
    expect(out.every((b) => b.rms === 0)).toBe(true);
  });
});
