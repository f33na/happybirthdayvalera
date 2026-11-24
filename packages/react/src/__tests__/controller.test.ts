import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ListenerController, type ControllerConfig, type DictationSink } from '../core/controller';
import { behavior } from '../core/data';
import type { SpeechEngine } from '../core/engine';
import type * as MicModule from '../core/mic';
import type { MicBlock } from '../core/mic';

const mic = vi.hoisted(() => ({
  onBlock: null as null | ((block: MicBlock) => void),
  stop: vi.fn(),
}));
vi.mock('../core/mic', async (original) => ({
  ...(await original<typeof MicModule>()),
  openMic: vi.fn(async (onBlock: (block: MicBlock) => void) => {
    mic.onBlock = onBlock;
    return { stop: mic.stop };
  }),
}));

interface Call {
  pcm: Float32Array;
  resolve: (text: string) => void;
  reject: (error: Error) => void;
}

/** An engine whose calls stay open until the test answers them. */
function deferredEngine() {
  const calls: Call[] = [];
  const engine: SpeechEngine = {
    load: vi.fn(async () => undefined),
    transcribe: vi.fn(
      (pcm: Float32Array) =>
        new Promise<string>((resolve, reject) => {
          calls.push({ pcm, resolve, reject });
        }),
    ),
  };
  return { engine, calls };
}

const LOUD = 0.1;
const QUIET = 0.001;
const BLOCK = 320;
const feed = (rms: number, blocks: number): void => {
  for (let i = 0; i < blocks; i++) mic.onBlock?.({ pcm: new Float32Array(BLOCK).fill(rms), rms, ms: 20 });
};
/** Lets the promise chains of the controller run. */
const settle = () => vi.advanceTimersByTimeAsync(0);

function setup(patch: Partial<ControllerConfig> = {}) {
  const { engine, calls } = deferredEngine();
  const onResult = vi.fn();
  const onInterim = vi.fn();
  const config: ControllerConfig = {
    lang: 'en',
    model: 'tiny',
    engine,
    mode: 'continuous',
    interimEveryMs: 0,
    silenceMs: behavior.silenceMs,
    segmentSilenceMs: 700,
    autoStopMs: 8000,
    segmentPadBeforeMs: 200,
    segmentPadAfterMs: 300,
    maxSegmentMs: 28000,
    holdMs: 300,
    easterEgg: false,
    onResult,
    onInterim,
    ...patch,
  };
  const controller = new ListenerController(config);
  const events: string[] = [];
  const sink: DictationSink = {
    begin: () => events.push('begin'),
    draft: (t) => events.push(`draft:${t}`),
    commit: (t) => events.push(`commit:${t}`),
    clear: () => events.push('clear'),
  };
  controller.bind(sink);
  const listen = async () => {
    controller.start();
    await settle();
    expect(controller.getSnapshot().state).toBe('listening');
  };
  /** One spoken segment: 400 ms of speech and the pause that closes it. */
  const segment = () => {
    feed(LOUD, 20);
    feed(QUIET, 35);
  };
  return { controller, engine, calls, onResult, onInterim, events, listen, segment, state: () => controller.getSnapshot() };
}

beforeEach(() => {
  vi.useFakeTimers();
  mic.onBlock = null;
  mic.stop.mockClear();
});
afterEach(() => {
  vi.useRealTimers();
});

describe('continuous mode', () => {
  it('commits segments in order, one engine call at a time, and keeps the mic on', async () => {
    const t = setup();
    await t.listen();
    t.segment();
    t.segment();
    expect(t.calls).toHaveLength(1); // the second final waits for the first
    t.calls[0]?.resolve('Hello there.');
    await settle();
    expect(t.calls).toHaveLength(2);
    t.calls[1]?.resolve(' How are you? ');
    await settle();

    expect(t.onResult.mock.calls).toEqual([
      ['Hello there.', 'en'],
      ['How are you?', 'en'],
    ]);
    expect(t.state().text).toBe('Hello there. How are you?');
    expect(t.state().state).toBe('listening');
    expect(mic.stop).not.toHaveBeenCalled();
    expect(t.events).toEqual(['begin', 'commit:Hello there.', 'commit:How are you?']);
  });

  it('sends the segment audio with its padding', async () => {
    const t = setup();
    await t.listen();
    feed(QUIET, 50);
    t.segment();
    // 10 blocks of padding before, 20 of speech, 15 of padding after.
    expect(t.calls[0]?.pcm.length).toBe(45 * BLOCK);
  });

  it('fires nothing for an empty segment result and drops its draft', async () => {
    const t = setup();
    await t.listen();
    t.segment();
    t.calls[0]?.resolve('  ');
    await settle();
    expect(t.onResult).not.toHaveBeenCalled();
    expect(t.events).toEqual(['begin', 'clear']);
    expect(t.state().state).toBe('listening');
  });

  it('keeps listening after a failed segment and fails the session only when nothing was committed', async () => {
    const t = setup();
    await t.listen();
    t.segment();
    t.calls[0]?.reject(new Error('boom'));
    await settle();
    expect(t.state().state).toBe('listening');
    t.controller.stop();
    await settle();
    expect(t.state().state).toBe('error');
    expect(t.state().error).toBe('transcribe-failed');
  });

  it('on STOP closes the open segment, drains the queue in order, then goes idle with the session text', async () => {
    const t = setup({ easterEgg: true });
    await t.listen();
    t.segment();
    t.segment();
    feed(LOUD, 20); // the open segment, still speaking
    t.controller.stop();
    expect(t.state().state).toBe('transcribing');
    expect(mic.stop).toHaveBeenCalled(); // released at once, before the queue drains
    for (const [i, text] of ['One.', 'Two.', 'Happy birthday, Valera'].entries()) {
      expect(t.calls).toHaveLength(i + 1);
      t.calls[i]?.resolve(text);
      await settle();
    }
    expect(t.state().state).toBe('idle');
    expect(t.state().text).toBe('One. Two. Happy birthday, Valera');
    expect(t.state().nothingHeard).toBe(false);
    expect(t.state().celebrating).toBe(true); // every committed segment is checked
    expect(t.onResult).toHaveBeenCalledTimes(3);
  });

  it('on STOP with nothing said goes idle with "nothing heard" and no engine call', async () => {
    const t = setup();
    await t.listen();
    feed(QUIET, 100);
    t.controller.stop();
    expect(t.state().state).toBe('idle');
    expect(t.state().nothingHeard).toBe(true);
    expect(t.engine.transcribe).not.toHaveBeenCalled();
  });

  it('auto-stops after autoStopMs without speech, counted from the end of the last speech', async () => {
    const t = setup({ autoStopMs: 2000 });
    await t.listen();
    feed(LOUD, 20);
    feed(QUIET, 99); // 1980 ms after the speech: still on
    expect(t.state().state).toBe('listening');
    feed(QUIET, 1);
    expect(t.state().state).toBe('transcribing');
    expect(mic.stop).toHaveBeenCalled();
    t.calls[0]?.resolve('Done.');
    await settle();
    expect(t.state().state).toBe('idle');
    expect(t.state().text).toBe('Done.');
  });

  it('does not stop on maxDurationMs', async () => {
    const t = setup({ autoStopMs: 10 * 60_000 });
    await t.listen();
    vi.advanceTimersByTime(behavior.maxDurationMs + 1000);
    expect(t.state().state).toBe('listening');
  });
});

describe('drafts', () => {
  it('transcribes only the open segment, with the padding before it', async () => {
    const t = setup({ interimEveryMs: 1000 });
    await t.listen();
    t.segment(); // segment 1
    t.calls[0]?.resolve('First.');
    await settle();
    feed(QUIET, 40);
    feed(LOUD, 50); // segment 2 is open and speaking
    vi.advanceTimersByTime(1000);
    expect(t.calls).toHaveLength(2);
    expect(t.calls[1]?.pcm.length).toBe((10 + 50) * BLOCK);
    t.calls[1]?.resolve('second dra');
    await settle();
    expect(t.state().interim).toBe('second dra');
    expect(t.onInterim).toHaveBeenCalledWith('second dra', 'en');
    expect(t.events.at(-1)).toBe('draft:second dra');
  });

  it('skips the tick while there is no speech in the open segment', async () => {
    const t = setup({ interimEveryMs: 1000 });
    await t.listen();
    feed(QUIET, 50);
    vi.advanceTimersByTime(3000);
    expect(t.engine.transcribe).not.toHaveBeenCalled();
  });

  it('skips the tick while a final call runs or waits, and the final goes first', async () => {
    const t = setup({ interimEveryMs: 1000 });
    await t.listen();
    feed(LOUD, 20);
    vi.advanceTimersByTime(1000);
    expect(t.calls).toHaveLength(1); // a draft of segment 1 runs
    feed(QUIET, 35); // segment 1 closes: its final waits for the draft
    feed(LOUD, 20); // segment 2 speaks
    vi.advanceTimersByTime(3000);
    expect(t.calls).toHaveLength(1); // no draft while a final waits

    t.calls[0]?.resolve('draft of one');
    await settle();
    // Segment 1 is closed but not committed, so its late draft still shows.
    expect(t.state().interim).toBe('draft of one');
    expect(t.calls).toHaveLength(2); // the final, not a draft
    expect(t.calls[1]?.pcm.length).toBe(35 * BLOCK); // speech from the first block, so no padding before
    vi.advanceTimersByTime(1000);
    expect(t.calls).toHaveLength(2); // the final still runs

    t.calls[1]?.resolve('One.');
    await settle();
    expect(t.state().text).toBe('One.');
    expect(t.state().interim).toBe('');
    vi.advanceTimersByTime(1000);
    expect(t.calls).toHaveLength(3); // now a draft of segment 2
    t.calls[2]?.resolve('two');
    await settle();
    expect(t.state().interim).toBe('two');
    expect(t.events).toEqual(['begin', 'draft:draft of one', 'commit:One.', 'draft:two']);
  });

  it('drops a draft whose segment is already resolved', async () => {
    const t = setup({ interimEveryMs: 1000 });
    await t.listen();
    feed(LOUD, 20);
    vi.advanceTimersByTime(1000); // a draft of segment 1 runs
    t.controller.stop(); // segment 1 closes and its final waits
    t.calls[0]?.resolve('late');
    await settle();
    expect(t.state().interim).toBe('late'); // segment 1 is not committed yet
    t.calls[1]?.resolve('Final.');
    await settle();
    expect(t.state().state).toBe('idle');
    expect(t.state().interim).toBe('');
    expect(t.state().text).toBe('Final.');
  });

  it('clears the draft when its segment is committed', async () => {
    const t = setup({ interimEveryMs: 1000 });
    await t.listen();
    feed(LOUD, 20);
    vi.advanceTimersByTime(1000);
    t.calls[0]?.resolve('hel');
    await settle();
    expect(t.state().interim).toBe('hel');
    feed(QUIET, 35);
    t.calls[1]?.resolve('Hello.');
    await settle();
    expect(t.state().interim).toBe('');
    expect(t.events).toEqual(['begin', 'draft:hel', 'commit:Hello.']);
  });

  it('works over the single utterance in utterance mode', async () => {
    const t = setup({ mode: 'utterance', interimEveryMs: 1000 });
    await t.listen();
    feed(LOUD, 30);
    vi.advanceTimersByTime(1000);
    expect(t.calls[0]?.pcm.length).toBe(30 * BLOCK);
    t.calls[0]?.resolve('hi');
    await settle();
    expect(t.state().interim).toBe('hi');
    feed(QUIET, 75); // 1500 ms ends the utterance
    expect(t.state().state).toBe('transcribing');
    t.calls[1]?.resolve('Hi there.');
    await settle();
    expect(t.state().state).toBe('idle');
    expect(t.state().text).toBe('Hi there.');
    expect(t.onResult).toHaveBeenCalledOnce();
  });
});

describe('hold to talk', () => {
  it('a press longer than holdMs listens while held and stops on release', async () => {
    const t = setup();
    t.controller.pressStart();
    await settle();
    expect(t.state().state).toBe('listening');
    expect(t.state().holding).toBe(false);
    vi.advanceTimersByTime(300);
    expect(t.state().holding).toBe(true);
    feed(LOUD, 20);
    t.controller.pressEnd();
    expect(t.state().holding).toBe(false);
    expect(t.state().state).toBe('transcribing');
    t.controller.click(true); // the click of that press does nothing
    expect(t.state().state).toBe('transcribing');
  });

  it('a press shorter than holdMs is a tap: listening goes on', async () => {
    const t = setup();
    t.controller.pressStart();
    await settle();
    vi.advanceTimersByTime(100);
    t.controller.pressEnd();
    t.controller.click(true);
    vi.advanceTimersByTime(1000);
    expect(t.state().state).toBe('listening');
    expect(t.state().holding).toBe(false);
  });

  it('a press while listening is a plain tap that stops', async () => {
    const t = setup();
    await t.listen();
    t.controller.pressStart();
    vi.advanceTimersByTime(1000);
    expect(t.state().holding).toBe(false);
    t.controller.pressEnd();
    expect(t.state().state).toBe('listening');
    t.controller.click(true);
    expect(t.state().state).toBe('idle');
  });

  it('a keyboard click toggles', async () => {
    const t = setup();
    t.controller.click(false);
    await settle();
    expect(t.state().state).toBe('listening');
    t.controller.click(false);
    expect(t.state().state).toBe('idle');
  });
});

describe('non-speech output', () => {
  it('keeps the previous draft when a draft is only a sound tag', async () => {
    const t = setup({ interimEveryMs: 1000 });
    await t.listen();
    feed(LOUD, 50);
    vi.advanceTimersByTime(1000);
    t.calls[0]?.resolve('happy birth');
    await settle();
    vi.advanceTimersByTime(1000);
    t.calls[1]?.resolve(' [BLANK_AUDIO] ');
    await settle();
    expect(t.state().interim).toBe('happy birth');
    expect(t.onInterim).toHaveBeenCalledTimes(1);
    expect(t.events).toEqual(['begin', 'draft:happy birth']);
  });

  it('commits nothing for a final that is only a sound tag or a stock phrase, and drops its draft', async () => {
    const t = setup({ interimEveryMs: 1000 });
    await t.listen();
    feed(LOUD, 20);
    vi.advanceTimersByTime(1000);
    t.calls[0]?.resolve('thank');
    await settle();
    feed(QUIET, 35); // the segment closes
    await settle();
    t.calls[1]?.resolve('Thank you.');
    await settle();
    expect(t.state().text).toBe('');
    expect(t.state().interim).toBe('');
    expect(t.onResult).not.toHaveBeenCalled();
    expect(t.events).toEqual(['begin', 'draft:thank', 'clear']);

    t.segment();
    await settle();
    t.calls[2]?.resolve('[BLANK_AUDIO] привет');
    await settle();
    expect(t.state().text).toBe('привет');
    expect(t.events.at(-1)).toBe('commit:привет');
  });
});
