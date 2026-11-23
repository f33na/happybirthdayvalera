import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { SpeechEngine } from '../core/engine';
import type * as EngineModule from '../engines/whisper/engine';
import { whisperEngine } from '../engines/whisper/engine';
import type { WorkerRequest, WorkerResponse } from '../engines/whisper/protocol';
import { useListener } from '../react/useListener';

// The default engine is replaced by a spy, so the test sees the options that the hook hands to it.
vi.mock('../engines/whisper/engine', () => ({
  whisperEngine: vi.fn(
    (): SpeechEngine => ({ load: async () => undefined, transcribe: async () => '' }),
  ),
}));

afterEach(() => {
  cleanup();
  vi.mocked(whisperEngine).mockClear();
});

describe('useListener modelBaseUrl', () => {
  it('passes modelBaseUrl and wasmPaths to the default whisper engine', () => {
    renderHook(() => useListener({ lang: 'en', modelBaseUrl: 'http://x/', wasmPaths: '/ort/' }));
    expect(whisperEngine).toHaveBeenCalledWith({ wasmPaths: '/ort/', modelBaseUrl: 'http://x/' });
  });

  it('leaves modelBaseUrl undefined when it is not set', () => {
    renderHook(() => useListener({ lang: 'en' }));
    expect(whisperEngine).toHaveBeenCalledWith({ wasmPaths: undefined, modelBaseUrl: undefined });
  });

  it('does not create the default engine when the caller supplies one', () => {
    const own: SpeechEngine = { load: async () => undefined, transcribe: async () => '' };
    renderHook(() => useListener({ lang: 'en', engine: own, modelBaseUrl: 'http://x/' }));
    expect(whisperEngine).not.toHaveBeenCalled();
  });
});

describe('whisperEngine modelBaseUrl in the load message', () => {
  // The real engine, not the spy above.
  async function loadMessage(options: { modelBaseUrl?: string }): Promise<WorkerRequest> {
    const { whisperEngine: real } = await vi.importActual<typeof EngineModule>(
      '../engines/whisper/engine',
    );
    const sent: WorkerRequest[] = [];
    const worker = {
      onmessage: null as null | ((e: MessageEvent<unknown>) => void),
      onerror: null,
      postMessage(message: WorkerRequest) {
        sent.push(message);
        const reply: WorkerResponse = { type: 'ready' };
        queueMicrotask(() => worker.onmessage?.({ data: reply } as MessageEvent<unknown>));
      },
      terminate() {},
    };
    await real({ createWorker: () => worker as unknown as Worker, ...options }).load('tiny', () => undefined);
    return sent[0] as WorkerRequest;
  }

  it('adds a trailing slash to a base URL without one', async () => {
    expect(await loadMessage({ modelBaseUrl: 'http://x' })).toMatchObject({ modelBaseUrl: 'http://x/' });
  });

  it('does not double an existing trailing slash', async () => {
    expect(await loadMessage({ modelBaseUrl: 'http://x/' })).toMatchObject({ modelBaseUrl: 'http://x/' });
  });

  it('omits the field when the base URL is unset', async () => {
    expect(await loadMessage({})).not.toHaveProperty('modelBaseUrl');
  });

  it('omits the field when the base URL is an empty string', async () => {
    expect(await loadMessage({ modelBaseUrl: '' })).not.toHaveProperty('modelBaseUrl');
  });
});
