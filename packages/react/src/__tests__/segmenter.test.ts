import { describe, expect, it } from 'vitest';
import { Segmenter, type SegmenterOptions } from '../core/segmenter';

// 20 ms blocks of 320 samples at 16 kHz. Every sample of block i holds the value i, so a test can read
// which blocks a segment covers.
const BLOCK = 320;
const LOUD = 0.1;
const QUIET = 0.001;

const OPTIONS: SegmenterOptions = {
  sampleRate: 16000,
  thresholdRms: 0.012,
  minSpeechMs: 250,
  segmentSilenceMs: 700,
  padBeforeMs: 200,
  padAfterMs: 300,
  maxSegmentMs: 28000,
  autoStopMs: 8000,
};

function driver(options: Partial<SegmenterOptions> = {}) {
  const segmenter = new Segmenter({ ...OPTIONS, ...options });
  let index = 0;
  const closed: Float32Array[] = [];
  let autoStops = 0;
  const feed = (rms: number, blocks: number): void => {
    for (let i = 0; i < blocks; i++) {
      const step = segmenter.push({ pcm: new Float32Array(BLOCK).fill(index++), rms, ms: 20 });
      if (step.closed) closed.push(step.closed);
      if (step.autoStop) autoStops++;
    }
  };
  return { segmenter, closed, feed, autoStops: () => autoStops };
}

/** The first and the last block index in a segment, and its length in blocks. */
function span(audio: Float32Array | null | undefined): [number, number, number] {
  if (!audio) throw new Error('no audio');
  return [audio[0] ?? -1, audio[audio.length - 1] ?? -1, audio.length / BLOCK];
}

describe('Segmenter', () => {
  it('closes a segment after segmentSilenceMs, with the padding before and after the speech', () => {
    const d = driver();
    d.feed(QUIET, 50); // blocks 0..49
    d.feed(LOUD, 30); // speech: blocks 50..79
    d.feed(QUIET, 34);
    expect(d.closed).toHaveLength(0);
    d.feed(QUIET, 1); // the 35th quiet block makes 700 ms
    expect(d.closed).toHaveLength(1);
    // 200 ms (10 blocks) before block 50, 300 ms (15 blocks) after block 79.
    expect(span(d.closed[0])).toEqual([40, 94, 55]);
  });

  it('clamps the padding to the captured audio', () => {
    const d = driver();
    d.feed(LOUD, 20); // speech from the very first block
    expect(span(d.segmenter.flush())).toEqual([0, 19, 20]);
  });

  it('ignores a click shorter than minSpeechMs', () => {
    const d = driver();
    d.feed(LOUD, 12); // 240 ms
    d.feed(QUIET, 100);
    expect(d.closed).toHaveLength(0);
    expect(d.segmenter.speaking).toBe(false);
    expect(d.segmenter.flush()).toBeNull();
  });

  it('starts the next segment where the previous one ended, so no audio goes to two segments', () => {
    const d = driver({ segmentSilenceMs: 300, padBeforeMs: 400, padAfterMs: 300 });
    d.feed(LOUD, 20); // 0..19
    d.feed(QUIET, 15); // closes: pad after takes 20..34
    d.feed(LOUD, 20); // 35..54; a 400 ms pad would reach back to block 15
    d.feed(QUIET, 15);
    expect(d.closed.map(span)).toEqual([
      [0, 34, 35],
      [35, 69, 35],
    ]);
  });

  it('cuts a segment at maxSegmentMs and goes on in a new one', () => {
    const d = driver({ maxSegmentMs: 1000 });
    d.feed(LOUD, 120);
    expect(d.closed.map(span)).toEqual([
      [0, 49, 50],
      [50, 99, 50],
    ]);
    expect(d.segmenter.speaking).toBe(true);
    expect(span(d.segmenter.flush())).toEqual([100, 119, 20]);
  });

  it('gives a draft of the open segment only, from its padding to now', () => {
    const d = driver();
    expect(d.segmenter.draft()).toBeNull();
    d.feed(LOUD, 20); // segment 1: 0..19
    d.feed(QUIET, 35);
    const seq = d.segmenter.seq;
    d.feed(QUIET, 100);
    d.feed(LOUD, 30); // segment 2 starts at block 155
    expect(span(d.segmenter.draft())).toEqual([145, 184, 40]);
    expect(d.segmenter.seq).toBe(seq);
  });

  it('keeps memory bounded over a long quiet stretch', () => {
    const d = driver();
    d.feed(QUIET, 5000); // 100 s
    d.feed(LOUD, 20);
    // Only the padding survives from the quiet part.
    expect(span(d.segmenter.flush())).toEqual([4990, 5019, 30]);
  });

  it('auto-stops once after autoStopMs without speech, counted from the start', () => {
    const d = driver({ autoStopMs: 1000 });
    d.feed(QUIET, 49);
    expect(d.autoStops()).toBe(0);
    d.feed(QUIET, 1);
    expect(d.autoStops()).toBe(1);
    d.feed(QUIET, 100);
    expect(d.autoStops()).toBe(1);
  });

  it('counts the auto-stop from the end of the last speech, not from a click', () => {
    const d = driver({ autoStopMs: 1000 });
    d.feed(QUIET, 30);
    d.feed(LOUD, 20); // speech resets the count
    d.feed(QUIET, 49);
    expect(d.autoStops()).toBe(0);
    d.feed(LOUD, 5); // a click is not speech
    d.feed(QUIET, 1);
    expect(d.autoStops()).toBe(1);
  });
});
