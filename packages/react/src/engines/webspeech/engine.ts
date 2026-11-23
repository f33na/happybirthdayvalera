import type { Lang } from '../../core/data';
import { EngineError, type SpeechEngine } from '../../core/engine';

// The DOM lib has no types for the Web Speech API, so the small part that is used is declared here.
interface RecognitionAlternative {
  transcript: string;
}
interface RecognitionResult {
  isFinal: boolean;
  0: RecognitionAlternative;
}
interface RecognitionEvent {
  resultIndex: number;
  results: ArrayLike<RecognitionResult>;
}
interface Recognition {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  onresult: ((e: RecognitionEvent) => void) | null;
  onerror: ((e: { error: string }) => void) | null;
  onend: (() => void) | null;
  start(): void;
  stop(): void;
  abort(): void;
}
type RecognitionCtor = new () => Recognition;

const BCP47: Record<Lang, string> = { en: 'en-US', ru: 'ru-RU', de: 'de-DE', fr: 'fr-FR' };

function recognitionCtor(): RecognitionCtor | null {
  const w = globalThis as unknown as { SpeechRecognition?: RecognitionCtor; webkitSpeechRecognition?: RecognitionCtor };
  return w.SpeechRecognition ?? w.webkitSpeechRecognition ?? null;
}

export function isWebSpeechSupported(): boolean {
  return recognitionCtor() !== null;
}

/**
 * The light engine: no model download. The browser recognises the speech itself (Chrome and Safari send the
 * audio to their own service, which is not "local"). It listens to the microphone on its own, so it starts
 * in `begin()` and ignores the PCM buffer that `transcribe` receives.
 */
export function webSpeechEngine(): SpeechEngine {
  let recognition: Recognition | null = null;
  let finals: string[] = [];
  let failure: EngineError | null = null;
  let ended: Promise<void> = Promise.resolve();

  const teardown = (): void => {
    recognition?.abort();
    recognition = null;
  };

  return {
    load(_model, onProgress) {
      if (!recognitionCtor()) {
        return Promise.reject(new EngineError('unsupported', 'The Web Speech API is not available in this browser'));
      }
      onProgress(1);
      return Promise.resolve();
    },
    begin(lang) {
      const Ctor = recognitionCtor();
      if (!Ctor) return;
      teardown();
      finals = [];
      failure = null;
      const rec = new Ctor();
      rec.lang = BCP47[lang];
      rec.continuous = true;
      rec.interimResults = false;
      rec.onresult = (e) => {
        for (let i = e.resultIndex; i < e.results.length; i++) {
          const result = e.results[i];
          if (result?.isFinal) finals.push(result[0].transcript);
        }
      };
      rec.onerror = (e) => {
        // `no-speech` and `aborted` are normal ends, not failures.
        if (e.error !== 'no-speech' && e.error !== 'aborted') {
          failure = new EngineError('transcribe-failed', `Speech recognition error: ${e.error}`);
        }
      };
      ended = new Promise<void>((resolve) => {
        rec.onend = () => resolve();
      });
      recognition = rec;
      try {
        rec.start();
      } catch (error) {
        failure = new EngineError('transcribe-failed', 'Could not start speech recognition', { cause: error });
      }
    },
    async transcribe() {
      const rec = recognition;
      if (!rec) throw failure ?? new EngineError('transcribe-failed', 'Speech recognition was not started');
      rec.stop();
      // Some browsers never fire `end` after `stop`; do not hang the widget in "Transcribing".
      await Promise.race([ended, new Promise<void>((resolve) => setTimeout(resolve, 4000))]);
      recognition = null;
      if (failure && finals.length === 0) throw failure;
      return finals.join(' ').replace(/\s+/g, ' ').trim();
    },
    dispose: teardown,
  };
}
