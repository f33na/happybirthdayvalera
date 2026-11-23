import type { Lang } from '../../core/data';

// Whisper takes the language as a forced decoder token. Full names are what transformers.js documents.
export const WHISPER_LANGUAGE: Record<Lang, string> = {
  en: 'english',
  ru: 'russian',
  de: 'german',
  fr: 'french',
};

/** Options for `pipeline('automatic-speech-recognition', model, …)`. Shared by the worker and the Node accuracy test. */
export const PIPELINE_DTYPE = { encoder_model: 'q8', decoder_model_merged: 'q8' } as const;

export function transcribeOptions(lang: Lang) {
  return {
    language: WHISPER_LANGUAGE[lang],
    task: 'transcribe' as const,
    chunk_length_s: 30,
    return_timestamps: false,
  };
}

export interface FileProgress {
  status?: string;
  file?: string;
  loaded?: number;
  total?: number;
}

/**
 * Turns the per-file events of `progress_callback` into one fraction over all model files.
 * The result never goes down: a file that appears late adds to the total, and a falling bar looks broken.
 */
export function createProgressAggregator(): (event: FileProgress) => number {
  const files = new Map<string, { loaded: number; total: number }>();
  let best = 0;
  return (event) => {
    const { file, status } = event;
    if (file !== undefined) {
      if (status === 'progress' && typeof event.loaded === 'number' && typeof event.total === 'number') {
        files.set(file, { loaded: event.loaded, total: event.total });
      } else if (status === 'done') {
        const known = files.get(file);
        if (known) files.set(file, { loaded: known.total, total: known.total });
      }
    }
    let loaded = 0;
    let total = 0;
    for (const f of files.values()) {
      loaded += f.loaded;
      total += f.total;
    }
    // 0.99 at most: the caller reports 1 only after the pipeline is built.
    if (total > 0) best = Math.max(best, Math.min(0.99, loaded / total));
    return best;
  };
}

/** Trims and collapses whitespace in the model output. */
export function cleanTranscript(text: string): string {
  return text.replace(/\s+/g, ' ').trim();
}
