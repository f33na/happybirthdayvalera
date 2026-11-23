import { useEffect, useRef } from 'react';
import { t } from '../core/i18n';
import { isActive } from '../core/machine';
import type { Listener } from './useListener';

export interface MicButtonProps {
  listener: Listener;
  className?: string;
}

export function MicButton({ listener, className }: MicButtonProps) {
  const { state, lang, levelRef, toggle } = listener;
  const button = useRef<HTMLButtonElement>(null);
  const pressed = isActive(state);

  // `--hbv-level` drives the ring; it is written straight to the element so React never re-renders per frame.
  useEffect(() => {
    const el = button.current;
    if (!el) return undefined;
    if (state !== 'listening') {
      el.style.setProperty('--hbv-level', '0');
      return undefined;
    }
    let frame = requestAnimationFrame(function tick() {
      el.style.setProperty('--hbv-level', levelRef.current.toFixed(3));
      frame = requestAnimationFrame(tick);
    });
    return () => cancelAnimationFrame(frame);
  }, [state, levelRef]);

  return (
    <button
      ref={button}
      type="button"
      className={className ? `hbv-mic ${className}` : 'hbv-mic'}
      data-state={state}
      aria-pressed={pressed}
      aria-label={t(lang, pressed ? 'action.stop' : 'action.start')}
      onClick={toggle}
    >
      <span className="hbv-mic__ring" aria-hidden="true" />
      <svg className="hbv-mic__icon" viewBox="0 0 24 24" aria-hidden="true">
        <rect x="9" y="3" width="6" height="11" rx="3" fill="currentColor" />
        <path
          d="M5.5 11.5a6.5 6.5 0 0 0 13 0M12 18v3"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
        />
      </svg>
    </button>
  );
}
