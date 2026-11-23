/// <reference lib="webworker" />
// Module Web Worker entry. It speaks the protocol in shared/CONTRACT.md and nothing else.
// The same file, bundled with transformers.js, is shipped to the Dioxus web build.
import { env, pipeline } from '@huggingface/transformers';
import {
  cleanTranscript,
  EXPECTED_MODEL_FILES,
  createProgressAggregator,
  PIPELINE_DTYPE,
  transcribeOptions,
} from './pipeline-config';
import type { Lang } from '../../core/data';
import type { WorkerRequest, WorkerResponse } from './protocol';

interface Transcriber {
  (audio: Float32Array, options: ReturnType<typeof transcribeOptions>): Promise<{ text: string } | { text: string }[]>;
}

const post = (message: WorkerResponse): void => self.postMessage(message);

// By default models come from the Hugging Face Hub. All of them are kept in the browser Cache API.
env.allowLocalModels = false;
env.useBrowserCache = true;
const defaultLocalModelPath = env.localModelPath;

let loadedModel: string | null = null;
let transcriber: Transcriber | null = null;
let loading: { model: string; promise: Promise<void> } | null = null;

/** Self-hosted sizes: `content-length` of a `HEAD` request per expected file. A file that fails stays unknown. */
async function fetchHostedSizes(model: string, baseUrl: string): Promise<Map<string, number>> {
  const sizes = new Map<string, number>();
  await Promise.all(
    EXPECTED_MODEL_FILES.map(async (file) => {
      try {
        const response = await fetch(`${baseUrl}${model}/${file}`, { method: 'HEAD', signal: AbortSignal.timeout(4000) });
        const length = Number(response.headers.get('content-length'));
        if (response.ok && Number.isFinite(length) && length > 0) sizes.set(file, length);
      } catch {
        // Unknown sizes are a normal case, see above.
      }
    }),
  );
  return sizes;
}

/** File sizes from the Hub's model listing. Any failure (offline, blocked) gives an empty map: progress stays unknown. */
async function fetchFileSizes(model: string): Promise<Map<string, number>> {
  const sizes = new Map<string, number>();
  try {
    const response = await fetch(`${env.remoteHost}api/models/${model}?blobs=true`, {
      signal: AbortSignal.timeout(4000),
    });
    if (!response.ok) return sizes;
    const body: unknown = await response.json();
    const siblings = typeof body === 'object' && body !== null && 'siblings' in body ? body.siblings : null;
    if (!Array.isArray(siblings)) return sizes;
    for (const item of siblings as unknown[]) {
      if (typeof item !== 'object' || item === null) continue;
      const { rfilename, size } = item as { rfilename?: unknown; size?: unknown };
      if (typeof rfilename === 'string' && typeof size === 'number') sizes.set(rfilename, size);
    }
  } catch {
    // Unknown sizes are a normal case, see above.
  }
  return sizes;
}

async function load(model: string, wasmPaths?: string, modelBaseUrl?: string): Promise<void> {
  // Where onnxruntime-web fetches its .wasm and loader from; transformers.js defaults to jsDelivr.
  // Self-hosting puts them under `<base>ort/`, and an explicit `wasmPaths` still wins.
  const wasm = env.backends.onnx.wasm;
  const ortPaths = wasmPaths ?? (modelBaseUrl ? `${modelBaseUrl}ort/` : undefined);
  if (ortPaths && wasm) wasm.wasmPaths = ortPaths;
  // Models are read from `<base><model id>/…`, never from the Hub, when a base URL is given.
  env.allowLocalModels = modelBaseUrl !== undefined;
  env.allowRemoteModels = modelBaseUrl === undefined;
  env.localModelPath = modelBaseUrl ?? defaultLocalModelPath;
  if (loadedModel === model && transcriber) return;
  if (loading?.model === model) return loading.promise;
  const sizes = modelBaseUrl ? await fetchHostedSizes(model, modelBaseUrl) : await fetchFileSizes(model);
  const aggregate = createProgressAggregator(sizes);
  const promise = (async () => {
    transcriber = null;
    loadedModel = null;
    const built = await pipeline('automatic-speech-recognition', model, {
      dtype: PIPELINE_DTYPE,
      device: 'wasm',
      progress_callback: (event: object) => {
        const fraction = aggregate(event);
        post(fraction === null ? { type: 'progress' } : { type: 'progress', fraction });
      },
    });
    // The pipeline type is a large union; this worker only uses the call signature declared above.
    transcriber = built as unknown as Transcriber;
    loadedModel = model;
    post({ type: 'progress', fraction: 1 });
  })();
  loading = { model, promise };
  try {
    await promise;
  } finally {
    loading = null;
  }
}

async function transcribe(id: number, pcm: Float32Array, lang: Lang): Promise<void> {
  if (!transcriber) throw new Error('Model is not loaded');
  const output = await transcriber(pcm, transcribeOptions(lang));
  const first = Array.isArray(output) ? output[0] : output;
  post({ type: 'result', id, text: cleanTranscript(first?.text ?? '') });
}

// One request at a time: ONNX sessions are not re-entrant, and `load` must finish before `transcribe`.
let queue: Promise<void> = Promise.resolve();

self.onmessage = (event: MessageEvent<WorkerRequest>) => {
  const request = event.data;
  queue = queue.then(async () => {
    try {
      if (request.type === 'load') {
        await load(request.model, request.wasmPaths, request.modelBaseUrl);
        post({ type: 'ready' });
      } else {
        await transcribe(request.id, request.pcm, request.lang);
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      post(request.type === 'transcribe' ? { type: 'error', id: request.id, message } : { type: 'error', message });
    }
  });
};
