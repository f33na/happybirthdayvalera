import { act, cleanup, render, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { SpeechEngine } from '../core/engine';
import type * as MicModule from '../core/mic';
import { MicError, type MicBlock } from '../core/mic';
import { ListenerPill } from '../react/ListenerPill';
import { MicButton } from '../react/MicButton';
import { useListener, type UseListenerOptions } from '../react/useListener';

// The microphone is replaced by a handle that the test feeds by hand.
const mic = vi.hoisted(() => ({
  onBlock: null as null | ((block: MicBlock) => void),
  stop: vi.fn(),
  failWith: null as null | MicError,
}));
vi.mock('../core/mic', async (original) => ({
  ...(await original<typeof MicModule>()),
  openMic: vi.fn(async (onBlock: (block: MicBlock) => void) => {
    if (mic.failWith) throw mic.failWith;
    mic.onBlock = onBlock;
    return { stop: mic.stop };
  }),
}));

function fakeEngine(text = 'Happy birthday, Valera'): SpeechEngine & {
  load: ReturnType<typeof vi.fn>;
  transcribe: ReturnType<typeof vi.fn>;
} {
  return {
    load: vi.fn(async (_model: string, onProgress: (f: number) => void) => {
      onProgress(0.5);
    }),
    transcribe: vi.fn(async () => text),
  };
}

const block = (rms: number): MicBlock => ({ pcm: new Float32Array(320).fill(rms), rms, ms: 20 });
const feed = (rms: number, blocks: number) => {
  act(() => {
    for (let i = 0; i < blocks; i++) mic.onBlock?.(block(rms));
  });
};

function setup(options: UseListenerOptions) {
  return renderHook(() => useListener({ lang: 'en', ...options }));
}

beforeEach(() => {
  mic.onBlock = null;
  mic.failWith = null;
  mic.stop.mockClear();
});
afterEach(cleanup);

describe('useListener', () => {
  it('runs the happy path and reports the final text once', async () => {
    const engine = fakeEngine();
    const onResult = vi.fn();
    const { result } = setup({ engine, onResult, easterEgg: true });

    expect(result.current.state).toBe('idle');
    act(() => result.current.start());
    await waitFor(() => expect(result.current.state).toBe('listening'));
    expect(engine.load).toHaveBeenCalledWith('base', expect.any(Function));
    expect(result.current.progress).toBe(1);

    feed(0.1, 20); // speech
    feed(0.001, 75); // 1500 ms of silence ends the recording
    expect(mic.stop).toHaveBeenCalled(); // released before the transcription finishes
    await waitFor(() => expect(result.current.state).toBe('idle'));

    expect(engine.transcribe).toHaveBeenCalledOnce();
    const [pcm, lang] = engine.transcribe.mock.calls[0] as [Float32Array, string];
    expect(lang).toBe('en');
    expect(pcm.length).toBeGreaterThan(20 * 320);
    expect(result.current.text).toBe('Happy birthday, Valera');
    expect(onResult).toHaveBeenCalledExactlyOnceWith('Happy birthday, Valera', 'en');
    expect(result.current.celebrating).toBe(true);
    expect(result.current.nothingHeard).toBe(false);
  });

  it('shows the celebration on the pill and the mic while the state is idle (regression)', async () => {
    const { result } = setup({ engine: fakeEngine(), easterEgg: true });
    act(() => result.current.start());
    await waitFor(() => expect(result.current.state).toBe('listening'));
    feed(0.1, 20);
    act(() => result.current.stop());
    await waitFor(() => expect(result.current.celebrating).toBe(true));

    // A host that mounts the pill on `state !== 'idle'` would show nothing here. `pillVisible` must be true.
    expect(result.current.state).toBe('idle');
    expect(result.current.pillVisible).toBe(true);
    const listener = result.current;
    const { container } = render(
      <>
        {listener.pillVisible ? <ListenerPill listener={listener} /> : null}
        <MicButton listener={listener} />
      </>,
    );
    expect(container.querySelector('.hbv-pill')?.getAttribute('data-celebrate')).toBe('true');
    expect(container.querySelector('.hbv-pill__text')?.textContent).toBe('Happy birthday, Valera');
    expect(container.querySelector('.hbv-mic')?.getAttribute('data-celebrate')).toBe('true');

    act(() => result.current.start()); // a new START ends the celebration
    expect(result.current.celebrating).toBe(false);
  });

  it('reports an unknown progress as null', async () => {
    const engine = fakeEngine();
    let finish: () => void = () => undefined;
    engine.load.mockImplementation((_m: string, onProgress: (f: number | null) => void) => {
      onProgress(null);
      return new Promise<void>((resolve) => (finish = resolve));
    });
    const { result } = setup({ engine });
    act(() => result.current.start());
    await waitFor(() => expect(result.current.state).toBe('loading-model'));
    expect(result.current.progress).toBeNull();
    await act(async () => finish());
  });

  it('keeps the pill hidden while idle and quiet', () => {
    expect(setup({ engine: fakeEngine() }).result.current.pillVisible).toBe(false);
  });

  it('does not celebrate when the egg is off', async () => {
    const { result } = setup({ engine: fakeEngine() });
    act(() => result.current.start());
    await waitFor(() => expect(result.current.state).toBe('listening'));
    feed(0.1, 20);
    act(() => result.current.stop());
    await waitFor(() => expect(result.current.state).toBe('idle'));
    expect(result.current.celebrating).toBe(false);
  });

  it('says "nothing heard" and skips the engine when nobody spoke', async () => {
    const engine = fakeEngine();
    const onResult = vi.fn();
    const { result } = setup({ engine, onResult });
    act(() => result.current.start());
    await waitFor(() => expect(result.current.state).toBe('listening'));
    feed(0.001, 50);
    act(() => result.current.stop());
    expect(result.current.state).toBe('idle');
    expect(result.current.nothingHeard).toBe(true);
    expect(engine.transcribe).not.toHaveBeenCalled();
    expect(onResult).not.toHaveBeenCalled();
  });

  it('treats an empty transcript as nothing heard and does not call onResult', async () => {
    const onResult = vi.fn();
    const { result } = setup({ engine: fakeEngine('  '), onResult });
    act(() => result.current.start());
    await waitFor(() => expect(result.current.state).toBe('listening'));
    feed(0.1, 30);
    act(() => result.current.stop());
    await waitFor(() => expect(result.current.state).toBe('idle'));
    expect(result.current.nothingHeard).toBe(true);
    expect(onResult).not.toHaveBeenCalled();
  });

  it('cancels and releases the microphone on STOP while loading the model', async () => {
    const engine = fakeEngine();
    let finishLoad: () => void = () => undefined;
    engine.load.mockImplementation(() => new Promise<void>((resolve) => (finishLoad = resolve)));
    const { result } = setup({ engine });
    act(() => result.current.start());
    await waitFor(() => expect(result.current.state).toBe('loading-model'));
    act(() => result.current.stop());
    expect(result.current.state).toBe('idle');
    expect(mic.stop).toHaveBeenCalled();
    await act(async () => finishLoad());
    expect(result.current.state).toBe('idle'); // the late MODEL_READY is ignored
  });

  it.each([
    ['mic-denied'],
    ['mic-unavailable'],
    ['unsupported'],
  ] as const)('maps %s to an error state', async (code) => {
    mic.failWith = new MicError(code, 'x');
    const { result } = setup({ engine: fakeEngine() });
    act(() => result.current.start());
    await waitFor(() => expect(result.current.state).toBe('error'));
    expect(result.current.error).toBe(code);
    act(() => result.current.reset());
    expect(result.current.state).toBe('idle');
    expect(result.current.error).toBeNull();
  });

  it('maps a failing model load to model-failed and releases the mic', async () => {
    const engine = fakeEngine();
    engine.load.mockRejectedValue(new Error('network'));
    const { result } = setup({ engine });
    act(() => result.current.start());
    await waitFor(() => expect(result.current.state).toBe('error'));
    expect(result.current.error).toBe('model-failed');
    expect(mic.stop).toHaveBeenCalled();
  });

  it('maps a failing transcription to transcribe-failed', async () => {
    const engine = fakeEngine();
    engine.transcribe.mockRejectedValue(new Error('boom'));
    const { result } = setup({ engine });
    act(() => result.current.start());
    await waitFor(() => expect(result.current.state).toBe('listening'));
    feed(0.1, 30);
    act(() => result.current.stop());
    await waitFor(() => expect(result.current.state).toBe('error'));
    expect(result.current.error).toBe('transcribe-failed');
  });

  it('cycles and locks the language', () => {
    const { result, rerender } = renderHook((props: UseListenerOptions) => useListener(props), {
      initialProps: { engine: fakeEngine() } as UseListenerOptions,
    });
    expect(result.current.langLocked).toBe(false);
    act(() => result.current.setLang('de'));
    expect(result.current.lang).toBe('de');
    act(() => result.current.cycleLang());
    expect(result.current.lang).toBe('fr');
    rerender({ engine: fakeEngine(), lang: 'ru' });
    expect(result.current.lang).toBe('ru');
    expect(result.current.langLocked).toBe(true);
  });

  it('keeps levelRef a stable object', () => {
    const { result, rerender } = setup({ engine: fakeEngine() });
    const first = result.current.levelRef;
    rerender();
    expect(result.current.levelRef).toBe(first);
  });
});
