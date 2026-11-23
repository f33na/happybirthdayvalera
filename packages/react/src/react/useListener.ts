import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import { ListenerController, type ControllerConfig, type Snapshot } from '../core/controller';
import { behavior, type Lang, type ModelSize } from '../core/data';
import type { SpeechEngine } from '../core/engine';
import { detectLang, nextLang, persistLang as storeLang } from '../core/lang';
import type { ErrorCode, ListenerState } from '../core/machine';
import { webSpeechEngine } from '../engines/webspeech/engine';
import { whisperEngine } from '../engines/whisper/engine';
import type { Theme } from './slots';

export type EngineChoice = 'whisper' | 'webspeech' | SpeechEngine;

export interface UseListenerOptions {
  /** Force the language. The chip is hidden while it is set. */
  lang?: Lang;
  /** Remember the language chosen with the chip in localStorage. Default false. */
  persistLang?: boolean;
  /** `'whisper'` (default), `'webspeech'` or your own `SpeechEngine`. */
  engine?: EngineChoice;
  /** Whisper model size. Default `'base'`. */
  model?: ModelSize;
  /** Fires once per utterance with the final text. */
  onResult?: (text: string, lang: Lang) => void;
  /** Fires with partial text while listening (needs `interimEveryMs > 0`). */
  onInterim?: (text: string, lang: Lang) => void;
  /** Transcribe the buffer so far at this interval while listening. Default 0 (off). */
  interimEveryMs?: number;
  /** Silence after speech that ends the recording. Default 1500. */
  silenceMs?: number;
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
}

export interface Listener {
  state: ListenerState;
  error: ErrorCode | null;
  /** Model load progress, 0..1, or null while a file size is unknown. */
  progress: number | null;
  /** The last final text. */
  text: string;
  /** Partial text while listening. */
  interim: string;
  lang: Lang;
  setLang: (lang: Lang) => void;
  cycleLang: () => void;
  start: () => void;
  stop: () => void;
  toggle: () => void;
  reset: () => void;
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

function createEngine(choice: 'whisper' | 'webspeech', wasmPaths: string | undefined): SpeechEngine {
  return choice === 'webspeech' ? webSpeechEngine() : whisperEngine({ wasmPaths });
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
};

export function useListener(options: UseListenerOptions = {}): Listener {
  const {
    lang: forcedLang,
    persistLang = false,
    engine: engineChoice = 'whisper',
    model = behavior.models.default,
    interimEveryMs = behavior.interimEveryMs,
    silenceMs = behavior.silenceMs,
    easterEgg = false,
    preload = false,
    theme = 'default',
    wasmPaths,
  } = options;

  // `navigator` does not exist on the server, so the server snapshot is `en` and the client corrects it after hydration.
  const detected = useSyncExternalStore<Lang>(subscribeLanguages, () => detectLang(persistLang), () => 'en');
  const [chosen, setChosen] = useState<Lang | null>(null);
  const lang = forcedLang ?? chosen ?? detected;

  const engine = useMemo(
    () => (typeof engineChoice === 'string' ? createEngine(engineChoice, wasmPaths) : engineChoice),
    [engineChoice, wasmPaths],
  );
  // Engines made here are ours to dispose; a user-supplied one is not.
  const ownsEngine = typeof engineChoice === 'string';
  useEffect(() => (ownsEngine ? () => engine.dispose?.() : undefined), [engine, ownsEngine]);

  const config: ControllerConfig = {
    lang,
    model,
    engine,
    interimEveryMs,
    silenceMs,
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
    }),
    [controller],
  );
  const cycleLang = useCallback(() => setLang(nextLang(lang)), [lang, setLang]);

  return {
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
  };
}
