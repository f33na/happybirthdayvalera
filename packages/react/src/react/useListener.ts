import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import { ListenerController, type ControllerConfig, type Snapshot } from '../core/controller';
import { behavior, type Lang, type Mode, type ModelSize } from '../core/data';
import type { SpeechEngine } from '../core/engine';
import { detectLang, nextLang, persistLang as storeLang } from '../core/lang';
import type { ErrorCode, ListenerState } from '../core/machine';
import { webSpeechEngine } from '../engines/webspeech/engine';
import { whisperEngine } from '../engines/whisper/engine';
import type { Device } from '../engines/whisper/pipeline-config';
import type { Theme } from './slots';

export type EngineChoice = 'whisper' | 'webspeech' | SpeechEngine;

export interface UseListenerOptions {
  /** Force the language. The chip is hidden while it is set. */
  lang?: Lang;
  /** Remember the language chosen with the chip in localStorage. Default false. */
  persistLang?: boolean;
  /** `'whisper'` (default), `'webspeech'` or your own `SpeechEngine`. */
  engine?: EngineChoice;
  /** Whisper model size. Default `'tiny'`. */
  model?: ModelSize;
  /**
   * `'continuous'` (default): listen until a second tap, Esc or the auto-stop, and split the speech into
   * segments at pauses. `'utterance'`: one phrase, then stop.
   */
  mode?: Mode;
  /** Fires with the final text of every non-empty segment (utterance mode: once per utterance). */
  onResult?: (text: string, lang: Lang) => void;
  /** Fires with every non-empty draft of the open segment (needs `interimEveryMs > 0`). */
  onInterim?: (text: string, lang: Lang) => void;
  /** Transcribe the open segment as a draft at this interval while listening. Default 1000; 0 turns drafts off. */
  interimEveryMs?: number;
  /** Utterance mode: silence after speech that ends the recording. Default 1500. */
  silenceMs?: number;
  /** Continuous mode: silence after speech that ends a segment. Default 700. */
  segmentSilenceMs?: number;
  /** Continuous mode: no speech for this long stops listening. Default 8000. */
  autoStopMs?: number;
  /** Audio kept before the speech start of a segment. Default 200. */
  segmentPadBeforeMs?: number;
  /** Audio kept after the speech end of a segment. Default 300. */
  segmentPadAfterMs?: number;
  /** A longer segment is cut here (Whisper sees at most 30 s). Default 28000. */
  maxSegmentMs?: number;
  /** A press on the mic longer than this is hold-to-talk: release stops. Default 300. */
  holdMs?: number;
  /** Celebrate when the text contains a birthday phrase. Default false. */
  easterEgg?: boolean;
  /** Start loading the model on mount instead of on the first tap. Default false. */
  preload?: boolean;
  /** Look preset, rendered as `data-hbv-theme`. Default `'default'`. */
  theme?: Theme;
  /**
   * Where the onnxruntime `.wasm` files are served from (the default Whisper engine only).
   * Default: the jsDelivr CDN. Set it for a fully offline app.
   */
  wasmPaths?: string;
  /**
   * Self-hosted models (the default Whisper engine only): a base URL that serves `<model id>/…` and `ort/`.
   * Default: the Hugging Face Hub and jsDelivr. See the README, "Self-hosted models".
   */
  modelBaseUrl?: string;
  /**
   * Where Whisper runs (the default Whisper engine only). `'auto'` (default): WebGPU when the browser has an adapter
   * and `modelBaseUrl` is set, else wasm. See the README, "Speed".
   */
  device?: Device;
}

export interface Listener {
  state: ListenerState;
  error: ErrorCode | null;
  /** Model load progress, 0..1, or null while a file size is unknown. */
  progress: number | null;
  /** The committed text of the session so far (continuous mode: segments joined with one space). */
  text: string;
  /** The draft of the open segment while listening. */
  interim: string;
  lang: Lang;
  setLang: (lang: Lang) => void;
  cycleLang: () => void;
  start: () => void;
  stop: () => void;
  toggle: () => void;
  reset: () => void;
  /** Pointer down on the mic: starts at once from a rest state (a tap or the start of a hold). */
  pressStart: () => void;
  /** Pointer up or cancel on the mic: stops after a hold, keeps listening after a tap. */
  pressEnd: () => void;
  /** The click of the mic. `fromPointer`: the click ends a pointer press (`event.detail > 0`). */
  click: (fromPointer: boolean) => void;
  /** The pointer holds the mic longer than `holdMs`. */
  holding: boolean;
  /** A text field is bound (`useDictation`): the draft is shown in the field, not in the pill. */
  bound: boolean;
  celebrating: boolean;
  /** Smoothed input level 0..1. Written every frame without a re-render. */
  levelRef: { current: number };
  /** True after an utterance that produced no text, until the next start. */
  nothingHeard: boolean;
  /** True while `lang` is forced; the chip is hidden then. */
  langLocked: boolean;
  theme: Theme;
  /** `state !== 'idle' || nothingHeard || celebrating`. Mount the pill on this, or the easter egg stays hidden. */
  pillVisible: boolean;
}

function createEngine(
  choice: 'whisper' | 'webspeech',
  wasmPaths: string | undefined,
  modelBaseUrl: string | undefined,
  device: Device | undefined,
): SpeechEngine {
  return choice === 'webspeech' ? webSpeechEngine() : whisperEngine({ wasmPaths, modelBaseUrl, ...(device ? { device } : {}) });
}

function subscribeLanguages(onChange: () => void): () => void {
  window.addEventListener('languagechange', onChange);
  return () => window.removeEventListener('languagechange', onChange);
}

// The server renders the idle state; it has no microphone.
const SERVER_SNAPSHOT: Snapshot = {
  state: 'idle',
  error: null,
  progress: null,
  text: '',
  nothingHeard: false,
  interim: '',
  celebrating: false,
  holding: false,
};

export function useListener(options: UseListenerOptions = {}): Listener {
  return useListenerController(options, false).listener;
}

/** The hook behind `useListener` and `useDictation`: also hands out the controller, for binding a field. */
export function useListenerController(
  options: UseListenerOptions,
  bound: boolean,
): { listener: Listener; controller: ListenerController } {
  const {
    lang: forcedLang,
    persistLang = false,
    engine: engineChoice = 'whisper',
    model = behavior.models.default,
    mode = behavior.mode,
    interimEveryMs = behavior.interimEveryMs,
    silenceMs = behavior.silenceMs,
    segmentSilenceMs = behavior.segmentSilenceMs,
    autoStopMs = behavior.autoStopMs,
    segmentPadBeforeMs = behavior.segmentPadBeforeMs,
    segmentPadAfterMs = behavior.segmentPadAfterMs,
    maxSegmentMs = behavior.maxSegmentMs,
    holdMs = behavior.holdMs,
    easterEgg = false,
    preload = false,
    theme = 'default',
    wasmPaths,
    modelBaseUrl,
    device,
  } = options;

  // `navigator` does not exist on the server, so the server snapshot is `en` and the client corrects it after hydration.
  const detected = useSyncExternalStore<Lang>(subscribeLanguages, () => detectLang(persistLang), () => 'en');
  const [chosen, setChosen] = useState<Lang | null>(null);
  const lang = forcedLang ?? chosen ?? detected;

  const engine = useMemo(
    () => (typeof engineChoice === 'string' ? createEngine(engineChoice, wasmPaths, modelBaseUrl, device) : engineChoice),
    [engineChoice, wasmPaths, modelBaseUrl, device],
  );
  // Engines made here are ours to dispose; a user-supplied one is not.
  const ownsEngine = typeof engineChoice === 'string';
  useEffect(() => (ownsEngine ? () => engine.dispose?.() : undefined), [engine, ownsEngine]);

  const config: ControllerConfig = {
    lang,
    model,
    engine,
    mode,
    interimEveryMs,
    silenceMs,
    segmentSilenceMs,
    autoStopMs,
    segmentPadBeforeMs,
    segmentPadAfterMs,
    maxSegmentMs,
    holdMs,
    easterEgg,
    onResult: options.onResult,
    onInterim: options.onInterim,
  };
  const [controller] = useState(() => new ListenerController(config));
  // The controller reads its options from here, so callbacks never go stale. Effects run before any user event.
  useEffect(() => controller.configure(config));

  const snap = useSyncExternalStore(controller.subscribe, controller.getSnapshot, () => SERVER_SNAPSHOT);

  useEffect(() => {
    if (preload) controller.preload();
  }, [preload, controller, engine, model]);
  useEffect(() => () => controller.dispose(), [controller]);

  const setLang = useCallback(
    (next: Lang) => {
      setChosen(next);
      if (persistLang) storeLang(next);
    },
    [persistLang],
  );
  const actions = useMemo(
    () => ({
      start: () => controller.start(),
      stop: () => controller.stop(),
      toggle: () => controller.toggle(),
      reset: () => controller.reset(),
      pressStart: () => controller.pressStart(),
      pressEnd: () => controller.pressEnd(),
      click: (fromPointer: boolean) => controller.click(fromPointer),
    }),
    [controller],
  );
  const cycleLang = useCallback(() => setLang(nextLang(lang)), [lang, setLang]);

  const listener: Listener = {
    state: snap.state,
    error: snap.error,
    progress: snap.progress,
    text: snap.text,
    interim: snap.interim,
    lang,
    setLang,
    cycleLang,
    ...actions,
    celebrating: snap.celebrating,
    levelRef: controller.levelRef,
    nothingHeard: snap.nothingHeard,
    langLocked: forcedLang !== undefined,
    theme,
    pillVisible: snap.state !== 'idle' || snap.nothingHeard || snap.celebrating,
    holding: snap.holding,
    bound,
  };
  return { listener, controller };
}
