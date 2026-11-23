import { useEffect, useRef } from 'react';
import { wave } from '../core/data';
import type { ListenerState } from '../core/machine';
import { amplitude, strandPath } from '../core/wave';
import { useReducedMotion } from './useReducedMotion';

export interface WaveformProps {
  state: ListenerState;
  levelRef: { current: number };
}

const STRANDS = wave.strands.map((_, i) => i);
const VIEW_BOX = `0 0 ${wave.viewBoxWidth} ${wave.viewBoxHeight}`;

/**
 * Three strands drawn from `core/wave`. The `d` attributes are written in `requestAnimationFrame` through
 * refs, so the 60 fps animation never goes through React.
 */
export function Waveform({ state, levelRef }: WaveformProps) {
  const paths = useRef<(SVGPathElement | null)[]>([]);
  const reduced = useReducedMotion();

  useEffect(() => {
    const draw = (t: number, A: number): void => {
      STRANDS.forEach((i) => paths.current[i]?.setAttribute('d', strandPath(i, t, A)));
    };
    if (reduced) {
      // Static wave: time frozen at 0, drawn once per state change.
      draw(0, amplitude(state, 0, true));
      return undefined;
    }
    const origin = performance.now();
    let frame = requestAnimationFrame(function tick(now) {
      draw((now - origin) / 1000, amplitude(state, levelRef.current));
      frame = requestAnimationFrame(tick);
    });
    return () => cancelAnimationFrame(frame);
  }, [state, reduced, levelRef]);

  return (
    <svg className="hbv-wave" viewBox={VIEW_BOX} aria-hidden="true">
      {STRANDS.map((i) => (
        <path
          key={i}
          ref={(el) => {
            paths.current[i] = el;
          }}
          className="hbv-wave__strand"
          data-i={i}
          // Initial frame, so server markup and the first paint already show a wave.
          d={strandPath(i, 0, wave.restAmp)}
          style={{ strokeWidth: wave.strokeWidth, opacity: wave.strands[i]?.opacity }}
        />
      ))}
    </svg>
  );
}
