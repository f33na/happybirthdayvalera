import { behavior, type Lang } from '../../core/data';

// Whisper takes the language as a forced decoder token. Full names are what transformers.js documents.
export const WHISPER_LANGUAGE: Record<Lang, string> = {
  en: 'english',
  ru: 'russian',
  de: 'german',
  fr: 'french',
};

/** What the host asks for. `auto`: WebGPU when it is available and the models are self-hosted, else wasm. */
export type Device = 'auto' | 'webgpu' | 'wasm';
/** What actually runs. */
export type Backend = 'webgpu' | 'wasm';

/** Options for `pipeline('automatic-speech-recognition', model, …)` on wasm. Shared by the worker and the Node accuracy test. */
export const PIPELINE_DTYPE = { encoder_model: 'q8', decoder_model_merged: 'q8' } as const;

/**
 * WebGPU dtypes. The q8 files give garbage text on WebGPU and run slower than on wasm, and an fp16 encoder breaks
 * `base`. An fp32 encoder with an fp16 decoder is the fast and correct pair, but it is about 2.3 times the q8
 * download (tiny 92.5 MB instead of 41 MB), so `auto` uses it only with self-hosted models.
 */
export const WEBGPU_DTYPE = { encoder_model: 'fp32', decoder_model_merged: 'fp16' } as const;

export function dtypeFor(backend: Backend): typeof PIPELINE_DTYPE | typeof WEBGPU_DTYPE {
  return backend === 'webgpu' ? WEBGPU_DTYPE : PIPELINE_DTYPE;
}

/**
 * Models that `auto` runs on WebGPU. `base` passes the fixture keywords in all four languages there and is about
 * 7 times faster than on one wasm thread. `tiny` on WebGPU misses a French keyword that the q8 model finds (its
 * full-precision output splits "anniversaire"), and it is already fast on wasm threads. `small` is not measured.
 */
export const AUTO_WEBGPU_MODELS: ReadonlySet<string> = new Set([behavior.models.web.base]);

/** `webgpu` without an adapter falls back to wasm instead of failing. */
export function resolveBackend(requested: Device, hasAdapter: boolean, selfHosted: boolean, model: string): Backend {
  if (requested === 'wasm' || !hasAdapter) return 'wasm';
  if (requested === 'webgpu') return 'webgpu';
  return selfHosted && AUTO_WEBGPU_MODELS.has(model) ? 'webgpu' : 'wasm';
}

/**
 * onnxruntime-web threads. Threads need SharedArrayBuffer, which exists only on a cross-origin-isolated page
 * (COOP `same-origin` + COEP `require-corp` or `credentialless`). More than 4 was slower on an 8-core M2:
 * the extra threads land on the efficiency cores.
 */
export function wasmThreads(isolated: boolean, cores: number | undefined): number {
  return isolated ? Math.max(1, Math.min(4, cores ?? 1)) : 1;
}

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
}

/** The two large files that transformers.js downloads for `PIPELINE_DTYPE` (`q8` is the `_quantized` suffix). */
export const EXPECTED_MODEL_FILES = ['onnx/encoder_model_quantized.onnx', 'onnx/decoder_model_merged_quantized.onnx'];
/** The same for `WEBGPU_DTYPE` (fp32 has no suffix). */
export const WEBGPU_MODEL_FILES = ['onnx/encoder_model.onnx', 'onnx/decoder_model_merged_fp16.onnx'];

export function expectedModelFiles(backend: Backend): readonly string[] {
  return backend === 'webgpu' ? WEBGPU_MODEL_FILES : EXPECTED_MODEL_FILES;
}

/**
 * Turns the per-file events of `progress_callback` into one fraction over all model files, or `null` while
 * a size is unknown. The sizes come from the Hub's file list, or from `HEAD` requests when the models are
 * self-hosted, not from the events: a CDN that sends no `content-length` makes transformers.js report
 * `total === loaded`, which would show 99 % for minutes.
 * The two big model files are counted from the start, so the denominator does not jump when they begin.
 * Any other file counts only when its size is known. The self-hosted path has sizes for the two big files
 * only, and the small config and tokenizer files would otherwise keep the fraction unknown to the end.
 * The value never goes down, and stays below 1 until the caller reports that the pipeline is built.
 */
export function createProgressAggregator(
  sizes: ReadonlyMap<string, number>,
  expected: readonly string[] = EXPECTED_MODEL_FILES,
): (event: FileProgress) => number | null {
  const files = new Map<string, { loaded: number; total: number | null }>();
  const track = (file: string): { loaded: number; total: number | null } => {
    let entry = files.get(file);
    if (!entry) {
      entry = { loaded: 0, total: sizes.get(file) ?? null };
      files.set(file, entry);
    }
    return entry;
  };
  for (const file of expected) track(file);
  let best = 0;

  return (event) => {
    if (event.file !== undefined && (files.has(event.file) || sizes.has(event.file))) {
      const entry = track(event.file);
      if (event.status === 'progress' && typeof event.loaded === 'number') entry.loaded = event.loaded;
      else if (event.status === 'done' && entry.total !== null) entry.loaded = entry.total;
    }
    let loaded = 0;
    let total = 0;
    for (const f of files.values()) {
      // A file of unknown size makes the whole fraction unknown. Showing a guess would be a fake percentage.
      if (f.total === null || f.total <= 0) return null;
      loaded += Math.min(f.loaded, f.total);
      total += f.total;
    }
    best = Math.max(best, Math.min(0.99, loaded / total));
    return best;
  };
}


/** Adds the trailing `/` that the self-hosted layout (`<base><model id>/…`, `<base>ort/`) relies on. */
export function normalizeBaseUrl(url: string): string {
  return url.endsWith('/') ? url : `${url}/`;
}

/** Trims and collapses whitespace in the model output. */
export function cleanTranscript(text: string): string {
  return text.replace(/\s+/g, ' ').trim();
}
