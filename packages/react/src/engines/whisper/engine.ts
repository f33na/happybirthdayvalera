import { behavior, type Lang, type ModelSize } from '../../core/data';
import { EngineError, type SpeechEngine } from '../../core/engine';
import { isWorkerResponse, type WorkerRequest } from './protocol';

export interface WhisperEngineOptions {
  /**
   * URL of a bundled `whisper-worker.js` (the file shipped as `happybirthdayvalera/whisper-worker.js`).
   * Use it when your bundler does not pick up the default worker, or to serve the worker from a CDN.
   */
  workerUrl?: string | URL;
  /** Full control: return your own module worker. Wins over `workerUrl`. */
  createWorker?: () => Worker;
  /**
   * Base URL that serves the onnxruntime-web `.wasm` and `.mjs` files (from `@huggingface/transformers/dist`).
   * Default: the jsDelivr CDN. Set it, and cache the model files, for a fully offline app.
   */
  wasmPaths?: string;
}

/**
 * The `new Worker(new URL(…, import.meta.url), { type: 'module' })` form is the one that Vite, webpack 5
 * and Next detect. At build time `scripts/build.mjs` swaps the source path for the shipped file name.
 */
function defaultWorker(): Worker {
  return new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
}

interface Pending {
  resolve: (text: string) => void;
  reject: (error: Error) => void;
}

export function whisperEngine(options: WhisperEngineOptions = {}): SpeechEngine {
  let worker: Worker | null = null;
  let loadedModel: ModelSize | null = null;
  let nextId = 1;
  const pending = new Map<number, Pending>();
  let loadWaiter: { resolve: () => void; reject: (e: Error) => void; onProgress: (f: number | null) => void } | null = null;

  const failAll = (error: Error): void => {
    for (const p of pending.values()) p.reject(error);
    pending.clear();
    loadWaiter?.reject(error);
    loadWaiter = null;
  };

  const spawn = (): Worker => {
    if (!options.createWorker && typeof Worker === 'undefined') {
      throw new EngineError('unsupported', 'Web Workers are not available');
    }
    const created = options.createWorker
      ? options.createWorker()
      : options.workerUrl
        ? new Worker(options.workerUrl, { type: 'module' })
        : defaultWorker();
    created.onmessage = (event: MessageEvent<unknown>) => {
      const message = event.data;
      if (!isWorkerResponse(message)) return;
      switch (message.type) {
        case 'progress':
          loadWaiter?.onProgress(message.fraction ?? null);
          break;
        case 'ready':
          loadWaiter?.resolve();
          loadWaiter = null;
          break;
        case 'result':
          pending.get(message.id)?.resolve(message.text.trim());
          pending.delete(message.id);
          break;
        case 'error':
          if (message.id === undefined) {
            loadWaiter?.reject(new EngineError('model-failed', message.message));
            loadWaiter = null;
          } else {
            pending.get(message.id)?.reject(new EngineError('transcribe-failed', message.message));
            pending.delete(message.id);
          }
          break;
      }
    };
    created.onerror = (event: ErrorEvent) => {
      // Fires when the worker script fails to load or throws at top level.
      failAll(new EngineError('model-failed', event.message || 'The speech worker failed to start'));
    };
    return created;
  };

  const send = (message: WorkerRequest, transfer: Transferable[] = []): void => {
    worker ??= spawn();
    worker.postMessage(message, transfer);
  };

  return {
    load(model, onProgress) {
      if (loadedModel === model && worker) {
        onProgress(1);
        return Promise.resolve();
      }
      return new Promise<void>((resolve, reject) => {
        loadWaiter?.reject(new EngineError('model-failed', 'Superseded by another load'));
        loadWaiter = {
          resolve: () => {
            loadedModel = model;
            resolve();
          },
          reject,
          onProgress,
        };
        try {
          send({ type: 'load', model: behavior.models.web[model], wasmPaths: options.wasmPaths });
        } catch (error) {
          loadWaiter = null;
          reject(error instanceof Error ? error : new EngineError('model-failed', String(error)));
        }
      });
    },
    transcribe(pcm: Float32Array, lang: Lang) {
      return new Promise<string>((resolve, reject) => {
        const id = nextId++;
        pending.set(id, { resolve, reject });
        try {
          send({ type: 'transcribe', id, pcm, lang }, [pcm.buffer]);
        } catch (error) {
          pending.delete(id);
          reject(error instanceof Error ? error : new EngineError('transcribe-failed', String(error)));
        }
      });
    },
    dispose() {
      failAll(new EngineError('transcribe-failed', 'Engine disposed'));
      worker?.terminate();
      worker = null;
      loadedModel = null;
    },
  };
}
