/// <reference lib="webworker" />
// Module Web Worker entry. It speaks the protocol in shared/CONTRACT.md and nothing else.
// The same file, bundled with transformers.js, is shipped to the Dioxus web build.
import { env, pipeline } from '@huggingface/transformers';
import {
  cleanTranscript,
  createProgressAggregator,
  dtypeFor,
  expectedModelFiles,
  resolveBackend,
  transcribeOptions,
  wasmThreads,
  type Backend,
  type Device,
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

/** `<model id>@<backend>` of the loaded pipeline. */
let loadedKey: string | null = null;
let loadedBackend: Backend = 'wasm';
let transcriber: Transcriber | null = null;
let loading: { key: string; promise: Promise<Backend> } | null = null;

/** True when the browser gives a WebGPU adapter. The DOM lib has no WebGPU types, so the shape is checked here. */
async function hasWebGpu(): Promise<boolean> {
  const gpu: unknown = Reflect.get(navigator, 'gpu');
  if (typeof gpu !== 'object' || gpu === null || !('requestAdapter' in gpu) || typeof gpu.requestAdapter !== 'function') {
    return false;
  }
  try {
    const adapter: unknown = await gpu.requestAdapter();
    return adapter !== null && adapter !== undefined;
  } catch {
    return false;
  }
}

/** Self-hosted sizes: `content-length` of a `HEAD` request per expected file. A file that fails stays unknown. */
async function fetchHostedSizes(model: string, baseUrl: string, files: readonly string[]): Promise<Map<string, number>> {
  const sizes = new Map<string, number>();
  await Promise.all(
    files.map(async (file) => {
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

async function build(model: string, backend: Backend, modelBaseUrl: string | undefined): Promise<void> {
  const files = expectedModelFiles(backend);
  const sizes = modelBaseUrl ? await fetchHostedSizes(model, modelBaseUrl, files) : await fetchFileSizes(model);
  const aggregate = createProgressAggregator(sizes, files);
  const built = await pipeline('automatic-speech-recognition', model, {
    dtype: dtypeFor(backend),
    device: backend,
    // onnxruntime warns on every WebGPU session that shape ops run on the CPU. That is by design, and the warning
    // shows up as a console error in the host app; only real errors are logged.
    session_options: { logSeverityLevel: 3 },
    progress_callback: (event: object) => {
      const fraction = aggregate(event);
      post(fraction === null ? { type: 'progress' } : { type: 'progress', fraction });
    },
  });
  // The pipeline type is a large union; this worker only uses the call signature declared above.
  transcriber = built as unknown as Transcriber;
}

async function load(model: string, wasmPaths?: string, modelBaseUrl?: string, device: Device = 'auto'): Promise<Backend> {
  // Where onnxruntime-web fetches its .wasm and loader from; transformers.js defaults to jsDelivr.
  // Self-hosting puts them under `<base>ort/`, and an explicit `wasmPaths` still wins.
  const wasm = env.backends.onnx.wasm;
  const ortPaths = wasmPaths ?? (modelBaseUrl ? `${modelBaseUrl}ort/` : undefined);
  if (wasm) {
    if (ortPaths) wasm.wasmPaths = ortPaths;
    // Read once, when the first session starts. Without isolation onnxruntime runs on one thread anyway.
    wasm.numThreads = wasmThreads(self.crossOriginIsolated, navigator.hardwareConcurrency);
  }
  // Models are read from `<base><model id>/…`, never from the Hub, when a base URL is given.
  env.allowLocalModels = modelBaseUrl !== undefined;
  env.allowRemoteModels = modelBaseUrl === undefined;
  env.localModelPath = modelBaseUrl ?? defaultLocalModelPath;
  const wanted = resolveBackend(device, device !== 'wasm' && (await hasWebGpu()), modelBaseUrl !== undefined, model);
  const key = `${model}@${wanted}`;
  if (loadedKey === key && transcriber) return loadedBackend;
  if (loading?.key === key) return loading.promise;
  const promise = (async (): Promise<Backend> => {
    transcriber = null;
    loadedKey = null;
    let backend = wanted;
    try {
      await build(model, backend, modelBaseUrl);
    } catch (error) {
      // `auto` must never be worse than wasm: missing fp32/fp16 files or a GPU failure fall back to q8 on wasm.
      if (backend !== 'webgpu' || device !== 'auto') throw error;
      backend = 'wasm';
      await build(model, backend, modelBaseUrl);
    }
    loadedKey = key;
    loadedBackend = backend;
    post({ type: 'progress', fraction: 1 });
    return backend;
  })();
  loading = { key, promise };
  try {
    return await promise;
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
        const device = await load(request.model, request.wasmPaths, request.modelBaseUrl, request.device);
        post({ type: 'ready', device });
      } else {
        await transcribe(request.id, request.pcm, request.lang);
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      post(request.type === 'transcribe' ? { type: 'error', id: request.id, message } : { type: 'error', message });
    }
  });
};
