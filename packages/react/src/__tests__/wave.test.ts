import { describe, expect, it } from 'vitest';
import { wave } from '../core/data';
import { amplitude, smoothLevel, strandPath } from '../core/wave';
import { readRootJson } from './helpers';

interface Fixture {
  cases: { strand: number; t: number; level: number; state: 'listening'; d: string }[];
}
const fixture = readRootJson<Fixture>('shared/wave-fixture.json');

describe('wave fixture', () => {
  it('has a case for every strand', () => {
    expect(fixture.cases.map((c) => c.strand)).toEqual([0, 1, 2]);
  });

  it.each(fixture.cases)('strand $strand matches the reference path exactly', (c) => {
    expect(strandPath(c.strand, c.t, amplitude(c.state, c.level))).toBe(c.d);
  });
});

describe('amplitude', () => {
  it('follows the level while listening and rests otherwise', () => {
    expect(amplitude('listening', 0)).toBeCloseTo(wave.idleAmp);
    expect(amplitude('listening', 1)).toBeCloseTo(1);
    expect(amplitude('idle', 1)).toBe(wave.restAmp);
    expect(amplitude('transcribing', 0.5)).toBe(wave.restAmp);
  });

  it('uses the reduced-motion amplitude in every state', () => {
    expect(amplitude('listening', 1, true)).toBe(wave.reducedMotionAmp);
    expect(amplitude('idle', 0, true)).toBe(wave.reducedMotionAmp);
  });
});

describe('smoothLevel', () => {
  it('attacks faster than it releases', () => {
    const up = smoothLevel(0, 1);
    const down = 1 - smoothLevel(1, 0);
    expect(up).toBeCloseTo(wave.attack);
    expect(down).toBeCloseTo(wave.release);
    expect(up).toBeGreaterThan(down);
  });

  it('clamps the target to 0..1', () => {
    expect(smoothLevel(0.9, 10)).toBeLessThanOrEqual(1);
    expect(smoothLevel(0, -1)).toBe(0);
  });
});

describe('strandPath', () => {
  it('has `points` samples, starts with M and pins the ends to the centre line', () => {
    const d = strandPath(0, 1.23, 1);
    const parts = d.split(' L');
    expect(parts).toHaveLength(wave.points);
    expect(parts[0]).toBe(`M0.00 ${(wave.viewBoxHeight / 2).toFixed(2)}`);
    expect(d).not.toContain('-0.00');
  });
});
