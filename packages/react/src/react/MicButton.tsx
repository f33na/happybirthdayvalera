import { forwardRef, useEffect, useImperativeHandle, useRef, type MouseEventHandler, type ReactNode } from 'react';
import { t } from '../core/i18n';
import { isActive } from '../core/machine';
import { Slot } from './Slot';
import { cx, type MicClassNames, type Theme } from './slots';
import type { Listener } from './useListener';

export interface MicButtonProps {
  listener: Listener;
  /** Overrides `listener.theme` for this component. */
  theme?: Theme;
  className?: string;
  /** Classes for the parts, added after the contract class names. */
  classNames?: MicClassNames;
  /** Render the only child element (for example a shadcn `<Button>`) instead of a `<button>` of our own. */
  asChild?: boolean;
  children?: ReactNode;
}

export interface MicButtonDomProps {
  type: 'button';
  'aria-pressed': boolean;
  'aria-label': string;
  'data-state': Listener['state'];
  'data-hbv-theme': Theme;
  onClick: MouseEventHandler;
}

/** The aria, data and click props of the mic button, for hosts that render their own element. */
export function micButtonProps(listener: Listener, theme: Theme = listener.theme): MicButtonDomProps {
  const pressed = isActive(listener.state);
  return {
    type: 'button',
    'aria-pressed': pressed,
    'aria-label': t(listener.lang, pressed ? 'action.stop' : 'action.start'),
    'data-state': listener.state,
    'data-hbv-theme': theme,
    onClick: listener.toggle,
  };
}

export const MicButton = forwardRef<HTMLButtonElement, MicButtonProps>(function MicButton(
  { listener, theme, className, classNames, asChild = false, children },
  forwarded,
) {
  const { state, levelRef } = listener;
  const element = useRef<HTMLButtonElement>(null);
  // The ref is set before the handle is read; the cast only drops the initial null.
  useImperativeHandle(forwarded, () => element.current as HTMLButtonElement);

  // `--hbv-level` drives the ring; it is written straight to the element so React never re-renders per frame.
  useEffect(() => {
    const el = element.current;
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

  const inner = (
    <>
      <span className={cx('hbv-mic__ring', classNames?.ring)} aria-hidden="true" />
      <svg className={cx('hbv-mic__icon', classNames?.icon)} viewBox="0 0 24 24" aria-hidden="true">
        <rect x="9" y="3" width="6" height="11" rx="3" fill="currentColor" />
        <path
          d="M5.5 11.5a6.5 6.5 0 0 0 13 0M12 18v3"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
        />
      </svg>
    </>
  );
  const props = { ...micButtonProps(listener, theme), className: cx('hbv-mic', className, classNames?.root) };

  if (asChild) {
    return (
      <Slot {...props} slotRef={element} inner={inner}>
        {children}
      </Slot>
    );
  }
  return (
    <button ref={element} {...props}>
      {inner}
    </button>
  );
});
