import { describe, expect, it, vi } from 'vitest';
import { whisperEngine } from '../engines/whisper/engine';
import { cleanTranscript, createProgressAggregator, WHISPER_LANGUAGE } from '../engines/whisper/pipeline-config';
import { isWorkerResponse, type WorkerRequest, type WorkerResponse } from '../engines/whisper/protocol';

/** A scripted stand-in for the module worker. */
class FakeWorker {
  onmessage: ((e: MessageEvent<unknown>) => void) | null = null;
  onerror: ((e: ErrorEvent) => void) | null = null;
  sent: WorkerRequest[] = [];
  transfers: Transferable[][] = [];
  terminated = false;
  constructor(private readonly script: (req: WorkerRequest, reply: (m: WorkerResponse) => void) => void) {}
  postMessage(message: WorkerRequest, transfer: Transferable[] = []): void {
    this.sent.push(message);
    this.transfers.push(transfer);
    queueMicrotask(() => this.script(message, (m) => this.onmessage?.({ data: m } as MessageEvent<unknown>)));
  }
  terminate(): void {
    this.terminated = true;
  }
}

const happy = (req: WorkerRequest, reply: (m: WorkerResponse) => void): void => {
  if (req.type === 'load') {
    reply({ type: 'progress', fraction: 0.5 });
    reply({ type: 'ready' });
  } else {
    reply({ type: 'result', id: req.id, text: '  hello  ' });
  }
};

function engineWith(script = happy) {
  const worker = new FakeWorker(script);
  const engine = whisperEngine({ createWorker: () => worker as unknown as Worker });
  return { engine, worker };
}

describe('whisperEngine', () => {
  it('loads the model id from behavior.json and forwards progress', async () => {
    const { engine, worker } = engineWith();
    const onProgress = vi.fn();
    await engine.load('base', onProgress);
    expect(worker.sent[0]).toEqual({ type: 'load', model: 'onnx-community/whisper-base' });
    expect(onProgress).toHaveBeenCalledWith(0.5);
  });

  it('passes wasmPaths in the load message', async () => {
    const worker = new FakeWorker(happy);
    const engine = whisperEngine({ createWorker: () => worker as unknown as Worker, wasmPaths: '/ort/' });
    await engine.load('tiny', () => undefined);
    expect(worker.sent[0]).toEqual({ type: 'load', model: 'onnx-community/whisper-tiny', wasmPaths: '/ort/' });
  });

  it('does not load the same model twice', async () => {
    const { engine, worker } = engineWith();
    await engine.load('tiny', () => undefined);
    const onProgress = vi.fn();
    await engine.load('tiny', onProgress);
    expect(worker.sent.filter((m) => m.type === 'load')).toHaveLength(1);
    expect(onProgress).toHaveBeenCalledWith(1);
  });

  it('transfers the pcm buffer and trims the result', async () => {
    const { engine, worker } = engineWith();
    await engine.load('base', () => undefined);
    const pcm = new Float32Array(1600);
    const text = await engine.transcribe(pcm, 'ru');
    expect(text).toBe('hello');
    const request = worker.sent[1];
    expect(request).toMatchObject({ type: 'transcribe', lang: 'ru' });
    expect(worker.transfers[1]).toEqual([pcm.buffer]);
  });

  it('matches results to requests by id', async () => {
    const replies: (() => void)[] = [];
    const { engine } = engineWith((req, reply) => {
      if (req.type === 'load') reply({ type: 'ready' });
      else replies.unshift(() => reply({ type: 'result', id: req.id, text: `r${req.id}` }));
    });
    await engine.load('base', () => undefined);
    const a = engine.transcribe(new Float32Array(1), 'en');
    const b = engine.transcribe(new Float32Array(1), 'en');
    await Promise.resolve();
    await Promise.resolve();
    replies.forEach((r) => r()); // answers in reverse order
    expect(await Promise.all([a, b])).toEqual(['r1', 'r2']);
  });

  it('rejects a load error and a transcribe error separately', async () => {
    const failing = engineWith((req, reply) => {
      reply(req.type === 'load' ? { type: 'error', message: 'no network' } : { type: 'error', id: req.id, message: 'bad audio' });
    });
    await expect(failing.engine.load('base', () => undefined)).rejects.toThrow('no network');
    await expect(failing.engine.transcribe(new Float32Array(1), 'en')).rejects.toThrow('bad audio');
  });

  it('terminates the worker on dispose and starts a new one on the next load', async () => {
    const workers: FakeWorker[] = [];
    const engine = whisperEngine({
      createWorker: () => {
        const w = new FakeWorker(happy);
        workers.push(w);
        return w as unknown as Worker;
      },
    });
    await engine.load('base', () => undefined);
    engine.dispose?.();
    expect(workers[0]?.terminated).toBe(true);
    await engine.load('base', () => undefined);
    expect(workers).toHaveLength(2);
  });
});

describe('createProgressAggregator with self-hosted sizes', () => {
  const sizes = new Map([
    ['onnx/encoder_model_quantized.onnx', 100],
    ['onnx/decoder_model_merged_quantized.onnx', 100],
  ]);

  // Known bug (found live in a browser): a small file that has no HEAD size makes the fraction unknown for good.
  // Remove `.fails` with the fix.
  it('keeps a real fraction after an event for a file that has no known size', () => {
    const aggregate = createProgressAggregator(sizes);
    aggregate({ status: 'initiate', file: 'config.json' });
    expect(aggregate({ status: 'progress', file: 'onnx/encoder_model_quantized.onnx', loaded: 100 })).toBe(0.5);
  });
});

describe('worker protocol', () => {
  it('accepts exactly the contract messages', () => {
    expect(isWorkerResponse({ type: 'progress', fraction: 0.1 })).toBe(true);
    expect(isWorkerResponse({ type: 'ready' })).toBe(true);
    expect(isWorkerResponse({ type: 'result', id: 1, text: 'x' })).toBe(true);
    expect(isWorkerResponse({ type: 'error', message: 'x' })).toBe(true);
    expect(isWorkerResponse({ type: 'result', id: '1', text: 'x' })).toBe(false);
    expect(isWorkerResponse({ type: 'other' })).toBe(false);
    expect(isWorkerResponse(null)).toBe(false);
  });

  it('names the four languages for Whisper', () => {
    expect(WHISPER_LANGUAGE).toEqual({ en: 'english', ru: 'russian', de: 'german', fr: 'french' });
  });
});

describe('createProgressAggregator', () => {
  const sizes = new Map([
    ['onnx/encoder_model_quantized.onnx', 100],
    ['onnx/decoder_model_merged_quantized.onnx', 300],
    ['config.json', 10],
  ]);
  const ENC = 'onnx/encoder_model_quantized.onnx';
  const DEC = 'onnx/decoder_model_merged_quantized.onnx';

  it('counts the two big files from the start, from the real sizes', () => {
    const progress = createProgressAggregator(sizes);
    expect(progress({})).toBe(0);
    expect(progress({ status: 'progress', file: ENC, loaded: 50 })).toBeCloseTo(50 / 400);
    expect(progress({ status: 'progress', file: DEC, loaded: 150 })).toBeCloseTo(200 / 400);
  });

  it('never goes down when a small file appears', () => {
    const progress = createProgressAggregator(sizes);
    progress({ status: 'progress', file: ENC, loaded: 100 });
    const before = progress({ status: 'progress', file: DEC, loaded: 100 });
    expect(progress({ status: 'initiate', file: 'config.json' })).toBeGreaterThanOrEqual(before ?? 0);
  });

  it('is null while any file has no known size, even if events claim 100 %', () => {
    const progress = createProgressAggregator(new Map([[ENC, 100]]));
    expect(progress({ status: 'progress', file: ENC, loaded: 100 })).toBeNull();
    expect(progress({ status: 'progress', file: DEC, loaded: 5e6 })).toBeNull();
  });

  it('is null when the size list is missing (offline)', () => {
    expect(createProgressAggregator(new Map())({ status: 'progress', file: ENC, loaded: 1 })).toBeNull();
  });

  it('counts a finished file as fully loaded and stays below 1', () => {
    const progress = createProgressAggregator(sizes);
    progress({ status: 'done', file: ENC });
    expect(progress({ status: 'done', file: DEC })).toBe(0.99);
  });
});

describe('cleanTranscript', () => {
  it('trims and collapses whitespace', () => {
    expect(cleanTranscript('  Happy   birthday \n Valera ')).toBe('Happy birthday Valera');
  });
});
