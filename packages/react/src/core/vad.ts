export interface VadOptions {
  thresholdRms: number;
  minSpeechMs: number;
  silenceMs: number;
}

export type VadSignal = 'speech-start' | 'silence' | null;

export interface Vad {
  /** Feeds one RMS block that lasted `blockMs`. Each signal fires at most once. */
  push(rms: number, blockMs: number): VadSignal;
  readonly speechStarted: boolean;
}

export function createVad(options: VadOptions): Vad {
  let loudMs = 0;
  let quietMs = 0;
  let started = false;
  let done = false;

  return {
    get speechStarted() {
      return started;
    },
    push(rms, blockMs) {
      if (done) return null;
      const loud = rms >= options.thresholdRms;
      if (!started) {
        // Several loud blocks in a row are needed; one click or cough must not count.
        loudMs = loud ? loudMs + blockMs : 0;
        if (loudMs >= options.minSpeechMs) {
          started = true;
          quietMs = 0;
          return 'speech-start';
        }
        return null;
      }
      quietMs = loud ? 0 : quietMs + blockMs;
      if (quietMs >= options.silenceMs) {
        done = true;
        return 'silence';
      }
      return null;
    },
  };
}
