import { behavior, type Lang, type ModelSize } from './data';
import { isBirthdayPhrase } from './easter';
import { EngineError, type SpeechEngine } from './engine';
import {
  initialMachineState,
  reduce,
  type ErrorCode,
  type MachineEvent,
  type MachineState,
} from './machine';
import { MicError, openMic, type MicBlock, type MicHandle } from './mic';
import { createVad, type Vad } from './vad';
import { smoothLevel } from './wave';

export interface ControllerConfig {
  lang: Lang;
  model: ModelSize;
  engine: SpeechEngine;
  interimEveryMs: number;
  silenceMs: number;
  easterEgg: boolean;
  onResult?: (text: string, lang: Lang) => void;
  onInterim?: (text: string, lang: Lang) => void;
}

export interface Snapshot extends MachineState {
  interim: string;
  celebrating: boolean;
}

const INITIAL: Snapshot = { ...initialMachineState, interim: '', celebrating: false };
// Below this much audio Whisper only hallucinates; treat it as "nothing heard".
const MIN_SAMPLES = Math.round(behavior.sampleRate * 0.3);

function concat(chunks: Float32Array[], total: number): Float32Array {
  const out = new Float32Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.length;
  }
  return out;
}

function errorCodeOf(error: unknown, fallback: ErrorCode): ErrorCode {
  if (error instanceof MicError) return error.code;
  if (error instanceof EngineError && error.code === 'unsupported') return 'unsupported';
  return fallback;
}

/**
 * Framework-free orchestration: microphone, VAD, engine and timers around the pure machine.
 * The React hook only subscribes to it. A `session` counter makes every async step ignore its result
 * once the user has stopped, reset or restarted.
 */
export class ListenerController {
  readonly levelRef = { current: 0 };

  private snap: Snapshot = INITIAL;
  private readonly listeners = new Set<() => void>();
  private session = 0;
  private mic: MicHandle | null = null;
  private vad: Vad | null = null;
  private chunks: Float32Array[] = [];
  private samples = 0;
  private rms = 0;
  private raf: number | null = null;
  private maxTimer: ReturnType<typeof setTimeout> | null = null;
  private interimTimer: ReturnType<typeof setInterval> | null = null;
  private interimBusy = false;
  private celebrateTimer: ReturnType<typeof setTimeout> | null = null;
  private loadedKey: { engine: SpeechEngine; model: ModelSize } | null = null;
  private loading: {
    engine: SpeechEngine;
    model: ModelSize;
    promise: Promise<void>;
    sinks: Set<(f: number | null) => void>;
  } | null = null;

  private cfg: ControllerConfig;

  constructor(initial: ControllerConfig) {
    this.cfg = initial;
  }

  /** Called by the hook after every render, so timers and async steps always see the latest options. */
  configure(next: ControllerConfig): void {
    this.cfg = next;
  }

  private config(): ControllerConfig {
    return this.cfg;
  }

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  getSnapshot = (): Snapshot => this.snap;

  private update(patch: Partial<Snapshot>): void {
    this.snap = { ...this.snap, ...patch };
    for (const l of this.listeners) l();
  }

  private dispatch(event: MachineEvent): void {
    const next = reduce(this.snap, event);
    if (next !== this.snap) this.update(next);
  }

  /** Starts loading the model without the microphone, so the first `start()` is faster. */
  preload(): void {
    void this.ensureModel().catch(() => undefined);
  }

  private ensureModel(onProgress?: (f: number | null) => void): Promise<void> {
    const { engine, model } = this.config();
    if (this.loadedKey?.engine === engine && this.loadedKey.model === model) return Promise.resolve();
    if (this.loading?.engine === engine && this.loading.model === model) {
      if (onProgress) this.loading.sinks.add(onProgress);
      return this.loading.promise;
    }
    const sinks = new Set<(f: number | null) => void>(onProgress ? [onProgress] : []);
    const promise = engine
      .load(model, (f) => sinks.forEach((sink) => sink(f)))
      .then(() => {
        this.loadedKey = { engine, model };
      })
      .finally(() => {
        if (this.loading?.promise === promise) this.loading = null;
      });
    this.loading = { engine, model, promise, sinks };
    return promise;
  }

  start(): void {
    if (this.snap.state !== 'idle' && this.snap.state !== 'error') return;
    const id = ++this.session;
    // A new START ends the celebration.
    if (this.celebrateTimer) clearTimeout(this.celebrateTimer);
    this.celebrateTimer = null;
    this.update({ interim: '', celebrating: false });
    this.dispatch({ type: 'START' });
    this.levelRef.current = 0;
    const stale = (): boolean => id !== this.session;

    openMic((block) => this.onBlock(id, block), stale)
      .then(async (handle) => {
        if (stale()) {
          handle.stop();
          return;
        }
        this.mic = handle;
        this.dispatch({ type: 'MIC_READY' });
        try {
          await this.ensureModel((fraction) => {
            if (!stale()) this.dispatch({ type: 'MODEL_PROGRESS', fraction });
          });
        } catch (error) {
          if (!stale()) {
            this.releaseMic();
            this.dispatch({ type: 'MODEL_FAILED', error: errorCodeOf(error, 'model-failed') });
          }
          return;
        }
        if (!stale()) this.beginListening(id);
      })
      .catch((error: unknown) => {
        if (!stale()) this.dispatch({ type: 'MIC_FAILED', error: errorCodeOf(error, 'mic-unavailable') });
      });
  }

  private beginListening(id: number): void {
    const cfg = this.config();
    this.dispatch({ type: 'MODEL_READY' });
    this.chunks = [];
    this.samples = 0;
    this.vad = createVad({
      thresholdRms: behavior.vadThresholdRms,
      minSpeechMs: behavior.minSpeechMs,
      silenceMs: cfg.silenceMs,
    });
    this.maxTimer = setTimeout(() => this.stop(), behavior.maxDurationMs);
    if (cfg.interimEveryMs > 0 && !cfg.engine.begin) {
      this.interimTimer = setInterval(() => this.interimTick(id), cfg.interimEveryMs);
    }
    cfg.engine.begin?.(cfg.lang);
    this.startLevelLoop();
  }

  private onBlock(id: number, block: MicBlock): void {
    if (id !== this.session) return;
    this.rms = block.rms;
    if (this.snap.state !== 'listening' || !this.vad) return;
    this.chunks.push(block.pcm);
    this.samples += block.pcm.length;
    if (this.vad.push(block.rms, block.ms) === 'silence') this.finish('SILENCE');
  }

  private startLevelLoop(): void {
    if (this.raf !== null || typeof requestAnimationFrame === 'undefined') return;
    const tick = (): void => {
      const listening = this.snap.state === 'listening';
      this.levelRef.current = smoothLevel(this.levelRef.current, listening ? this.rms : 0);
      if (!listening && this.levelRef.current < 0.001) {
        this.levelRef.current = 0;
        this.raf = null;
        return;
      }
      this.raf = requestAnimationFrame(tick);
    };
    this.raf = requestAnimationFrame(tick);
  }

  private interimTick(id: number): void {
    const cfg = this.config();
    if (id !== this.session || this.interimBusy || !this.vad?.speechStarted) return;
    this.interimBusy = true;
    cfg.engine
      .transcribe(concat(this.chunks, this.samples), cfg.lang)
      .then((text) => {
        if (id !== this.session || this.snap.state !== 'listening' || !text) return;
        this.update({ interim: text });
        cfg.onInterim?.(text, cfg.lang);
      })
      .catch(() => undefined)
      .finally(() => {
        this.interimBusy = false;
      });
  }

  stop(): void {
    const { state } = this.snap;
    if (state === 'requesting' || state === 'loading-model') {
      this.session++;
      this.releaseMic();
      this.dispatch({ type: 'STOP' });
    } else if (state === 'listening') {
      this.finish('STOP');
    }
  }

  toggle(): void {
    const { state } = this.snap;
    if (state === 'idle' || state === 'error') this.start();
    else this.stop();
  }

  /** Back to a clean idle: cancels anything running and forgets the last text and error. */
  reset(): void {
    this.cancelAll();
    this.update({ ...INITIAL });
  }

  /** Releases everything. The controller stays usable, so React StrictMode can mount it again. */
  dispose(): void {
    this.cancelAll();
    if (this.celebrateTimer) clearTimeout(this.celebrateTimer);
    this.celebrateTimer = null;
    this.update({ ...INITIAL });
  }

  private cancelAll(): void {
    this.session++;
    this.releaseMic();
    this.levelRef.current = 0;
    this.interimBusy = false;
  }

  private releaseMic(): void {
    if (this.maxTimer) clearTimeout(this.maxTimer);
    if (this.interimTimer) clearInterval(this.interimTimer);
    this.maxTimer = null;
    this.interimTimer = null;
    this.mic?.stop();
    this.mic = null;
    this.rms = 0;
  }

  private finish(event: 'STOP' | 'SILENCE'): void {
    const id = this.session;
    const cfg = this.config();
    const spoke = this.vad?.speechStarted ?? false;
    const pcm = concat(this.chunks, this.samples);
    this.chunks = [];
    this.samples = 0;
    this.vad = null;
    this.dispatch({ type: event });
    // Leaving `listening` releases the microphone now, before the transcription ends.
    this.releaseMic();

    // Engines that listen on their own decide for themselves; for the others silence is not worth a call.
    if (!cfg.engine.begin && (!spoke || pcm.length < MIN_SAMPLES)) {
      this.dispatch({ type: 'RESULT', text: '' });
      this.update({ interim: '' });
      return;
    }
    cfg.engine
      .transcribe(pcm, cfg.lang)
      .then((text) => {
        if (id !== this.session) return;
        this.dispatch({ type: 'RESULT', text });
        this.update({ interim: '' });
        const final = text.trim();
        if (final) this.afterResult(final, cfg);
      })
      .catch(() => {
        if (id === this.session) this.dispatch({ type: 'TRANSCRIBE_FAILED' });
      });
  }

  private afterResult(text: string, cfg: ControllerConfig): void {
    cfg.onResult?.(text, cfg.lang);
    if (cfg.easterEgg && isBirthdayPhrase(text)) {
      this.update({ celebrating: true });
      if (this.celebrateTimer) clearTimeout(this.celebrateTimer);
      this.celebrateTimer = setTimeout(() => this.update({ celebrating: false }), behavior.easterEggMs);
    }
  }
}
