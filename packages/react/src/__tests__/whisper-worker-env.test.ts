import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { WorkerRequest, WorkerResponse } from '../engines/whisper/protocol';

// A mock of transformers.js: `env` is a plain object, `pipeline` reports one progress event per model file.
const MODEL_FILES = ['onnx/encoder_model_quantized.onnx', 'onnx/decoder_model_merged_quantized.onnx'];
const tf = vi.hoisted(() => ({
  env: {} as Record<string, unknown>,
  // Records the env at the moment the pipeline is built, which is when transformers.js reads it.
  seen: null as null | Record<string, unknown>,
  failPipeline: false,
  // Small files that transformers.js reports before the big ones. The worker has no size for them.
  smallFiles: [] as string[],
}));
vi.mock('@huggingface/transformers', () => ({
  get env() {
    return tf.env;
  },
  pipeline: vi.fn(async (_task: string, _model: string, options: { progress_callback: (e: object) => void }) => {
    tf.seen = {
      allowLocalModels: tf.env.allowLocalModels,
      allowRemoteModels: tf.env.allowRemoteModels,
      localModelPath: tf.env.localModelPath,
      wasmPaths: (tf.env.backends as { onnx: { wasm: { wasmPaths?: string } } }).onnx.wasm.wasmPaths,
    };
    if (tf.failPipeline) throw new Error('boom');
    for (const file of tf.smallFiles) options.progress_callback({ status: 'initiate', file });
    for (const file of MODEL_FILES) options.progress_callback({ status: 'progress', file, loaded: 50 });
    return async () => ({ text: ' ok ' });
  }),
}));

const HOSTED_SIZE = 100;

async function startWorker() {
  vi.resetModules();
  tf.env = {
    allowLocalModels: true,
    allowRemoteModels: true,
    useBrowserCache: false,
    remoteHost: 'https://huggingface.co/',
    localModelPath: '/models/',
    backends: { onnx: { wasm: {} as { wasmPaths?: string } } },
  };
  tf.seen = null;
  const posted: WorkerResponse[] = [];
  vi.spyOn(self, 'postMessage').mockImplementation(((message: WorkerResponse) => {
    posted.push(message);
  }) as typeof self.postMessage);
  await import('../engines/whisper/worker');
  const send = async (request: WorkerRequest): Promise<void> => {
    const before = posted.length;
    (self.onmessage as (e: MessageEvent<WorkerRequest>) => void)({ data: request } as MessageEvent<WorkerRequest>);
    await vi.waitFor(() => {
      const last = posted.slice(before).at(-1);
      expect(last?.type === 'ready' || last?.type === 'error').toBe(true);
    });
  };
  return { posted, send };
}

let fetched: { url: string; method: string }[];

beforeEach(() => {
  tf.failPipeline = false;
  tf.smallFiles = [];
  fetched = [];
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init?: RequestInit) => {
      fetched.push({ url, method: init?.method ?? 'GET' });
      if (init?.method === 'HEAD') return new Response(null, { headers: { 'content-length': String(HOSTED_SIZE) } });
      return new Response(JSON.stringify({ siblings: [] }));
    }),
  );
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('whisper worker env setup', () => {
  it('reads models only from the Hub when no base URL is given', async () => {
    const { send } = await startWorker();
    await send({ type: 'load', model: 'org/m' });
    expect(tf.seen).toMatchObject({ allowLocalModels: false, allowRemoteModels: true });
    expect(tf.seen?.wasmPaths).toBeUndefined();
  });

  it('reads models only from the base URL and never from the Hub when one is given', async () => {
    const { send } = await startWorker();
    await send({ type: 'load', model: 'org/m', modelBaseUrl: 'http://h/' });
    expect(tf.seen).toMatchObject({ allowLocalModels: true, allowRemoteModels: false, localModelPath: 'http://h/' });
  });

  it('defaults the onnxruntime files to <base>ort/', async () => {
    const { send } = await startWorker();
    await send({ type: 'load', model: 'org/m', modelBaseUrl: 'http://h/' });
    expect(tf.seen?.wasmPaths).toBe('http://h/ort/');
  });

  it('lets an explicit wasmPaths win over <base>ort/', async () => {
    const { send } = await startWorker();
    await send({ type: 'load', model: 'org/m', modelBaseUrl: 'http://h/', wasmPaths: '/my-ort/' });
    expect(tf.seen?.wasmPaths).toBe('/my-ort/');
  });

  it('takes sizes from HEAD requests to the base URL and reports real fractions', async () => {
    const { send, posted } = await startWorker();
    await send({ type: 'load', model: 'org/m', modelBaseUrl: 'http://h/' });
    expect(fetched.filter((f) => f.method === 'HEAD').map((f) => f.url).sort()).toEqual(
      MODEL_FILES.map((file) => `http://h/org/m/${file}`).sort(),
    );
    expect(fetched.some((f) => f.url.includes('huggingface.co'))).toBe(false);
    // 50 of 100 bytes in both files is half of the total.
    expect(posted).toContainEqual({ type: 'progress', fraction: 0.5 });
    expect(posted.at(-1)).toEqual({ type: 'ready' });
  });

  // Known bug (found live in a browser): the hosted sizes cover only the two big files, and the aggregator treats
  // any other file as an unknown size, so the progress never leaves the unknown state. Remove `.fails` with the fix.
  it('reports a real fraction while small files without a HEAD size are also loaded', async () => {
    tf.smallFiles = ['config.json', 'tokenizer.json', 'preprocessor_config.json'];
    const { send, posted } = await startWorker();
    await send({ type: 'load', model: 'org/m', modelBaseUrl: 'http://h/' });
    expect(posted).toContainEqual({ type: 'progress', fraction: 0.5 });
  });

  it('asks the Hub for sizes, not the base URL, when no base URL is given', async () => {
    const { send } = await startWorker();
    await send({ type: 'load', model: 'org/m' });
    expect(fetched.map((f) => f.url)).toEqual(['https://huggingface.co/api/models/org/m?blobs=true']);
  });

  it('keeps progress unknown when the HEAD requests fail', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new Error('offline'))));
    const { send, posted } = await startWorker();
    await send({ type: 'load', model: 'org/m', modelBaseUrl: 'http://h/' });
    expect(posted.filter((m) => m.type === 'progress' && m.fraction !== undefined && m.fraction < 1)).toEqual([]);
    expect(posted.at(-1)).toEqual({ type: 'ready' });
  });

  it('reports a load error without an id', async () => {
    tf.failPipeline = true;
    const { send, posted } = await startWorker();
    await send({ type: 'load', model: 'org/m', modelBaseUrl: 'http://h/' });
    expect(posted.at(-1)).toEqual({ type: 'error', message: 'boom' });
  });
});
