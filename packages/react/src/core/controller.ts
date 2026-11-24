import { behavior, type Lang, type Mode, type ModelSize } from './data';
import { isBirthdayPhrase } from './easter';
import { EngineError, type SpeechEngine } from './engine';
import {
  initialMachineState,
  isActive,
  reduce,
  type ErrorCode,
  type MachineEvent,
  type MachineState,
} from './machine';
import { MicError, openMic, type MicBlock, type MicHandle } from './mic';
import { Segmenter } from './segmenter';
import { cleanTranscript } from './transcript';
import { createVad, type Vad } from './vad';
import { smoothLevel } from './wave';

export interface ControllerConfig {
  lang: Lang;
  model: ModelSize;
  engine: SpeechEngine;
  mode: Mode;
  interimEveryMs: number;
  silenceMs: number;
  segmentSilenceMs: number;
  autoStopMs: number;
  segmentPadBeforeMs: number;
  segmentPadAfterMs: number;
  maxSegmentMs: number;
  holdMs: number;
  easterEgg: boolean;
  onResult?: (text: string, lang: Lang) => void;
  onInterim?: (text: string, lang: Lang) => void;
}

/** Receives the text of a session as it happens, for a bound text field. */
export interface DictationSink {
  /** A session starts: take the insertion point now. */
  begin(): void;
  /** The draft of the open segment changed. */
  draft(text: string): void;
  /** A segment is final. Replaces the draft, if one is shown. */
  commit(text: string): void;
  /** The draft is gone without a commit (the segment was empty, or the session ended). */
  clear(): void;
}

export interface Snapshot extends MachineState {
  interim: string;
  celebrating: boolean;
  /** The pointer has held the mic longer than `holdMs`; releasing it stops. */
  holding: boolean;
}

const INITIAL: Snapshot = { ...initialMachineState, interim: '', celebrating: false, holding: false };
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
 *
 * Engine calls go through one queue: one call at a time, a final call before any draft, and a draft tick
 * is skipped while a call runs or a final call waits. So a slow model throttles the drafts by itself.
 */
export class ListenerController {
  readonly levelRef = { current: 0 };

  private snap: Snapshot = INITIAL;
  private readonly listeners = new Set<() => void>();
  private session = 0;
  private mic: MicHandle | null = null;
  // Utterance mode, and any engine that captures its own audio: one buffer for the whole session.
  private vad: Vad | null = null;
  private chunks: Float32Array[] = [];
  private samples = 0;
  // Continuous mode with a PCM engine.
  private segmenter: Segmenter | null = null;
  private finals: Float32Array[] = [];
  private busy = false;
  private committed: string[] = [];
  /** Closed segments that are committed, empty or failed; segment N is resolved once this passes N. */
  private resolved = 0;
  private failed = false;
  private rms = 0;
  private raf: number | null = null;
  private maxTimer: ReturnType<typeof setTimeout> | null = null;
  private interimTimer: ReturnType<typeof setInterval> | null = null;
  private celebrateTimer: ReturnType<typeof setTimeout> | null = null;
  private holdTimer: ReturnType<typeof setTimeout> | null = null;
  /** The current pointer press started the session, so its click must not toggle it off again. */
  private pressStarted = false;
  private loadedKey: { engine: SpeechEngine; model: ModelSize } | null = null;
  private loading: {
    engine: SpeechEngine;
    model: ModelSize;
    promise: Promise<void>;
    sinks: Set<(f: number | null) => void>;
  } | null = null;

  private cfg: ControllerConfig;
  private sink: DictationSink | null = null;

  constructor(initial: ControllerConfig) {
    this.cfg = initial;
  }

  /** Called by the hook after every render, so timers and async steps always see the latest options. */
  configure(next: ControllerConfig): void {
    this.cfg = next;
  }

  /** Binds a text field (or unbinds with null). */
  bind(sink: DictationSink | null): void {
    this.sink = sink;
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
    this.sink?.begin();
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

  /** Continuous mode needs the PCM to cut segments; an engine that listens by itself gets one call at STOP. */
  private segmenting(cfg: ControllerConfig = this.config()): boolean {
    return cfg.mode === 'continuous' && !cfg.engine.begin;
  }

  private beginListening(id: number): void {
    const cfg = this.config();
    this.dispatch({ type: 'MODEL_READY' });
    this.chunks = [];
    this.samples = 0;
    this.finals = [];
    this.committed = [];
    this.resolved = 0;
    this.failed = false;
    this.vad = null;
    this.segmenter = null;
    if (cfg.mode === 'continuous') {
      // Also for an engine that listens by itself: the segmenter then only drives the auto-stop.
      this.segmenter = new Segmenter({
        sampleRate: behavior.sampleRate,
        thresholdRms: behavior.vadThresholdRms,
        minSpeechMs: behavior.minSpeechMs,
        segmentSilenceMs: cfg.segmentSilenceMs,
        padBeforeMs: cfg.segmentPadBeforeMs,
        padAfterMs: cfg.segmentPadAfterMs,
        maxSegmentMs: cfg.maxSegmentMs,
        autoStopMs: cfg.autoStopMs,
      });
    }
    if (!this.segmenting(cfg)) {
      this.vad = createVad({
        thresholdRms: behavior.vadThresholdRms,
        minSpeechMs: behavior.minSpeechMs,
        silenceMs: cfg.silenceMs,
      });
    }
    if (cfg.mode === 'utterance') this.maxTimer = setTimeout(() => this.stop(), behavior.maxDurationMs);
    if (cfg.interimEveryMs > 0 && !cfg.engine.begin) {
      this.interimTimer = setInterval(() => this.draftTick(id), cfg.interimEveryMs);
    }
    cfg.engine.begin?.(cfg.lang);
    this.startLevelLoop();
  }

  private onBlock(id: number, block: MicBlock): void {
    if (id !== this.session) return;
    this.rms = block.rms;
    if (this.snap.state !== 'listening') return;
    const cfg = this.config();
    if (this.segmenter) {
      const step = this.segmenter.push(block);
      if (this.segmenting(cfg)) {
        if (step.closed) this.enqueue(id, step.closed);
      } else {
        this.chunks.push(block.pcm);
        this.samples += block.pcm.length;
        this.vad?.push(block.rms, block.ms);
      }
      if (step.autoStop) this.stop();
      return;
    }
    this.chunks.push(block.pcm);
    this.samples += block.pcm.length;
    if (this.vad?.push(block.rms, block.ms) === 'silence') this.endListening('SILENCE');
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

  /** Transcribes the open segment (utterance mode: the whole buffer) as a draft, if the engine is free. */
  private draftTick(id: number): void {
    if (id !== this.session || this.snap.state !== 'listening' || this.busy || this.finals.length > 0) return;
    const segmenter = this.segmenting() ? this.segmenter : null;
    let pcm: Float32Array | null;
    if (segmenter) {
      pcm = segmenter.draft();
    } else {
      pcm = this.vad?.speechStarted ? concat(this.chunks, this.samples) : null;
    }
    if (!pcm || pcm.length < MIN_SAMPLES) return;
    const seq = segmenter?.seq ?? 0;
    const cfg = this.config();
    this.busy = true;
    cfg.engine
      .transcribe(pcm, cfg.lang)
      .then((text) => {
        // The segment may have closed meanwhile. Its final call cannot start before this one ends, so the draft
        // is still the newest text of a segment that is not committed: with a slow model it is the only draft.
        const { state } = this.snap;
        if (id !== this.session || (state !== 'listening' && state !== 'transcribing') || this.resolved > seq) return;
        // An empty draft (silence, a sound tag) keeps the previous draft on screen.
        const draft = cleanTranscript(text);
        if (!draft) return;
        this.update({ interim: draft });
        this.sink?.draft(draft);
        cfg.onInterim?.(draft, cfg.lang);
      })
      .catch(() => undefined)
      .finally(() => {
        if (id !== this.session) return;
        this.busy = false;
        this.pump(id);
      });
  }

  /** Queues the audio of a closed segment for its final call. */
  private enqueue(id: number, pcm: Float32Array): void {
    if (pcm.length < MIN_SAMPLES) {
      this.resolved++;
      this.dropDraft();
    } else {
      this.finals.push(pcm);
    }
    this.pump(id);
  }

  /** Runs the next final call, or ends the session once STOP has come and every segment is committed. */
  private pump(id: number): void {
    if (id !== this.session || this.busy) return;
    const pcm = this.finals.shift();
    if (!pcm) {
      if (this.snap.state === 'transcribing') this.finishSession();
      return;
    }
    const cfg = this.config();
    this.busy = true;
    cfg.engine
      .transcribe(pcm, cfg.lang)
      .then((text) => {
        if (id !== this.session) return;
        this.resolved++;
        this.commit(cleanTranscript(text), cfg);
      })
      .catch(() => {
        if (id !== this.session) return;
        this.resolved++;
        // One failed segment does not end a live session; the session fails only if nothing was committed.
        this.failed = true;
        this.dropDraft();
      })
      .finally(() => {
        if (id !== this.session) return;
        this.busy = false;
        this.pump(id);
      });
  }

  private dropDraft(): void {
    if (this.snap.interim) this.update({ interim: '' });
    this.sink?.clear();
  }

  private commit(text: string, cfg: ControllerConfig): void {
    if (!text) {
      this.dropDraft();
      return;
    }
    this.committed.push(text);
    this.update({ interim: '', text: this.committed.join(' ') });
    this.sink?.commit(text);
    cfg.onResult?.(text, cfg.lang);
    if (cfg.easterEgg && isBirthdayPhrase(text)) {
      this.update({ celebrating: true });
      if (this.celebrateTimer) clearTimeout(this.celebrateTimer);
      this.celebrateTimer = setTimeout(() => this.update({ celebrating: false }), behavior.easterEggMs);
    }
  }

  private finishSession(): void {
    this.sink?.clear();
    if (this.committed.length === 0 && this.failed) {
      this.dispatch({ type: 'TRANSCRIBE_FAILED' });
    } else {
      this.dispatch({ type: 'RESULT', text: this.committed.join(' ') });
    }
    this.update({ interim: '' });
  }

  stop(): void {
    const { state } = this.snap;
    if (state === 'requesting' || state === 'loading-model') {
      this.session++;
      this.releaseMic();
      this.dispatch({ type: 'STOP' });
    } else if (state === 'listening') {
      this.endListening('STOP');
    }
  }

  toggle(): void {
    const { state } = this.snap;
    if (state === 'idle' || state === 'error') this.start();
    else this.stop();
  }

  /**
   * Pointer down on the mic. From a rest state it starts at once, so no speech is lost while the press is
   * still a tap or a hold. In any other state the press is a plain tap, and its click stops.
   */
  pressStart(): void {
    this.clearHold();
    this.pressStarted = false;
    const { state } = this.snap;
    if (state !== 'idle' && state !== 'error') return;
    this.start();
    this.pressStarted = true;
    this.holdTimer = setTimeout(() => {
      this.holdTimer = null;
      if (isActive(this.snap.state)) this.update({ holding: true });
    }, this.config().holdMs);
  }

  /** Pointer up or cancel. After a hold it stops; after a tap the session goes on. */
  pressEnd(): void {
    const held = this.snap.holding;
    this.clearHold();
    if (held) this.stop();
  }

  /**
   * The click of the mic. A pointer click after `pressStart` started the session is ignored; any other click
   * (a keyboard Space or Enter, or a tap while busy) toggles.
   */
  click(fromPointer: boolean): void {
    const consumed = fromPointer && this.pressStarted;
    this.pressStarted = false;
    if (!consumed) this.toggle();
  }

  private clearHold(): void {
    if (this.holdTimer) clearTimeout(this.holdTimer);
    this.holdTimer = null;
    if (this.snap.holding) this.update({ holding: false });
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
    this.clearHold();
    this.levelRef.current = 0;
    this.busy = false;
    this.finals = [];
    this.sink?.clear();
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

  private endListening(event: 'STOP' | 'SILENCE'): void {
    const id = this.session;
    const cfg = this.config();
    const segmenter = this.segmenting(cfg) ? this.segmenter : null;
    const last = segmenter?.flush() ?? null;
    const spoke = this.vad?.speechStarted ?? false;
    const pcm = segmenter ? null : concat(this.chunks, this.samples);
    this.chunks = [];
    this.samples = 0;
    this.vad = null;
    this.segmenter = null;
    this.dispatch({ type: event });
    // Leaving `listening` releases the microphone now, before the transcription ends.
    this.releaseMic();

    if (pcm) {
      // Engines that listen on their own decide for themselves; for the others silence is not worth a call.
      if (!cfg.engine.begin && (!spoke || pcm.length < MIN_SAMPLES)) {
        this.dropDraft();
        this.finishSession();
        return;
      }
      this.finals.push(pcm);
    } else if (last) {
      this.enqueue(id, last);
      return;
    }
    this.pump(id);
  }
}
