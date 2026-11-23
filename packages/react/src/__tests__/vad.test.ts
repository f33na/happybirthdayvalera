import { describe, expect, it } from 'vitest';
import { createVad, type VadSignal } from '../core/vad';

const options = { thresholdRms: 0.012, minSpeechMs: 250, silenceMs: 1500 };
const LOUD = 0.1;
const QUIET = 0.001;

function run(sequence: [rms: number, blocks: number][], blockMs = 20): VadSignal[] {
  const vad = createVad(options);
  const signals: VadSignal[] = [];
  for (const [rms, blocks] of sequence) {
    for (let i = 0; i < blocks; i++) signals.push(vad.push(rms, blockMs));
  }
  return signals.filter((s) => s !== null);
}

describe('vad', () => {
  it('never stops before speech starts', () => {
    expect(run([[QUIET, 1000]])).toEqual([]);
  });

  it('starts speech after minSpeechMs of loud blocks', () => {
    const vad = createVad(options);
    for (let i = 0; i < 12; i++) expect(vad.push(LOUD, 20)).toBeNull(); // 240 ms
    expect(vad.push(LOUD, 20)).toBe('speech-start'); // 260 ms
    expect(vad.speechStarted).toBe(true);
  });

  it('ignores a click shorter than minSpeechMs', () => {
    expect(run([[LOUD, 5], [QUIET, 5], [LOUD, 5], [QUIET, 100]])).toEqual([]);
  });

  it('fires silence once after silenceMs below the threshold', () => {
    expect(run([[LOUD, 20], [QUIET, 74]])).toEqual(['speech-start']);
    expect(run([[LOUD, 20], [QUIET, 75]])).toEqual(['speech-start', 'silence']);
    expect(run([[LOUD, 20], [QUIET, 500]])).toEqual(['speech-start', 'silence']);
  });

  it('resets the silence timer when the voice comes back', () => {
    expect(run([[LOUD, 20], [QUIET, 70], [LOUD, 1], [QUIET, 70]])).toEqual(['speech-start']);
  });

  it('treats a level exactly at the threshold as speech', () => {
    const vad = createVad({ ...options, minSpeechMs: 20 });
    expect(vad.push(options.thresholdRms, 20)).toBe('speech-start');
  });

  it('honours a custom silenceMs', () => {
    const vad = createVad({ ...options, silenceMs: 100 });
    for (let i = 0; i < 13; i++) vad.push(LOUD, 20);
    const out: VadSignal[] = [];
    for (let i = 0; i < 5; i++) out.push(vad.push(QUIET, 20));
    expect(out).toEqual([null, null, null, null, 'silence']);
  });
});
