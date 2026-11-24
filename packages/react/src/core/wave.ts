import { wave, type WaveTokens } from './data';
import type { ListenerState } from './machine';

const fmt = (v: number): string => {
  const s = (Math.round(v * 100) / 100).toFixed(2);
  return s === '-0.00' ? '0.00' : s;
};

/** One smoothing step per animation frame. `rms` is the raw block level. */
export function smoothLevel(level: number, rms: number, tokens: WaveTokens = wave): number {
  const target = Math.min(1, Math.max(0, rms * tokens.levelGain));
  return level + (target - level) * (target > level ? tokens.attack : tokens.release);
}

export function amplitude(
  state: ListenerState,
  level: number,
  reducedMotion = false,
  tokens: WaveTokens = wave,
  celebrating = false,
): number {
  if (reducedMotion) return tokens.reducedMotionAmp;
  // The celebration keeps the wave lively after the recording, when the level is 0.
  if (celebrating) return tokens.idleAmp + (1 - tokens.idleAmp) * 0.6;
  if (state === 'listening') return tokens.idleAmp + (1 - tokens.idleAmp) * level;
  return tokens.restAmp;
}

/** SVG path of strand `i` at time `t` (seconds) and amplitude factor `A`, as specified in CONTRACT.md. */
export function strandPath(i: number, t: number, A: number, tokens: WaveTokens = wave): string {
  const s = tokens.strands[i];
  if (!s) throw new RangeError(`No wave strand ${i}`);
  const W = tokens.viewBoxWidth;
  const H = tokens.viewBoxHeight;
  const parts: string[] = [];
  for (let k = 0; k < tokens.points; k++) {
    const x = k / (tokens.points - 1);
    const env = Math.sin(Math.PI * x) ** 2;
    const y =
      H / 2 + (H / 2 - tokens.strokePad) * env * s.amp * A * Math.sin(2 * Math.PI * s.freq * x + s.phase + s.speed * t);
    parts.push(`${k === 0 ? 'M' : 'L'}${fmt(x * W)} ${fmt(y)}`);
  }
  return parts.join(' ');
}
