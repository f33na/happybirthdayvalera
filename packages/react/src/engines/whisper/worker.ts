/// <reference lib="webworker" />
// Module Web Worker entry. It speaks the protocol in shared/CONTRACT.md and nothing else.
// The same file, bundled with transformers.js, is shipped to the Dioxus web build.
import { env, pipeline } from '@huggingface/transformers';
import {
  cleanTranscript,
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

// Models come from the Hugging Face Hub and are kept in the browser Cache API for the next visit.
env.allowLocalModels = false;
env.useBrowserCache = true;

let loadedModel: string | null = null;
let transcriber: Transcriber | null = null;
let loading: { model: string; promise: Promise<void> } | null = null;

async function load(model: string): Promise<void> {
  if (loadedModel === model && transcriber) return;
  if (loading?.model === model) return loading.promise;
  const aggregate = createProgressAggregator();
  const promise = (async () => {
    transcriber = null;
    loadedModel = null;
    const built = await pipeline('automatic-speech-recognition', model, {
      dtype: PIPELINE_DTYPE,
      device: 'wasm',
      progress_callback: (event: object) => post({ type: 'progress', fraction: aggregate(event) }),
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
        await load(request.model);
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
