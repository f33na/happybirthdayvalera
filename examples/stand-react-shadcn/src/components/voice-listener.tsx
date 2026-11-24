'use client';

// shadcn registry item: a voice-input widget built from your own Button and the shadcn tokens.
// It uses the headless parts of `happybirthdayvalera` (the hook and the waveform) and no stylesheet of the
// package, so edit the classes freely.
import {
  Waveform,
  langName,
  langShort,
  micButtonProps,
  statusText,
  t,
  useDictation,
  type TextField,
  type UseListenerOptions,
} from 'happybirthdayvalera';
import * as React from 'react';

import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

// The package stylesheet puts its rules in `@layer components`, so these utilities win without `!`.
const STRANDS =
  "[&>path]:fill-none [&>path]:[stroke-linecap:round] [&>path]:[stroke-linejoin:round] [&>path[data-i='0']]:stroke-chart-1 [&>path[data-i='1']]:stroke-chart-2 [&>path[data-i='2']]:stroke-chart-3";

export type VoiceListenerProps = UseListenerOptions & {
  className?: string;
  /** A `<textarea>` or `<input>` to dictate into: drafts appear at the caret while the user speaks. */
  target?: React.RefObject<TextField | null>;
};

export function VoiceListener({ className, target, ...options }: VoiceListenerProps) {
  const listener = useDictation(target ?? null, options);
  const { state, lang, levelRef, celebrating } = listener;
  const working = state === 'listening' || state === 'transcribing';
  const ring = React.useRef<HTMLSpanElement>(null);

  // The ring follows the input level. It is written to the element, so React does not re-render per frame.
  React.useEffect(() => {
    const el = ring.current;
    if (!el || state !== 'listening') return undefined;
    let frame = requestAnimationFrame(function tick() {
      el.style.transform = `scale(${1 + levelRef.current * 0.45})`;
      frame = requestAnimationFrame(tick);
    });
    return () => {
      cancelAnimationFrame(frame);
      el.style.transform = '';
    };
  }, [state, levelRef]);

  const { 'data-hbv-theme': _theme, ...mic } = micButtonProps(listener, 'shadcn');
  const shown = working ? (listener.bound ? '' : listener.interim) : listener.text;

  return (
    <div className={cn('inline-flex items-center gap-2', className)}>
      {listener.pillVisible ? (
        <div
          role="group"
          aria-label={t(lang, 'label.widget')}
          data-state={state}
          data-celebrate={celebrating ? 'true' : 'false'}
          className={cn(
            'bg-popover text-popover-foreground inline-flex h-12 max-w-full items-center gap-2.5 rounded-full border px-3.5 text-sm font-medium shadow-md',
            state === 'error' && 'text-destructive',
            celebrating && 'ring-chart-2 ring-2 motion-safe:animate-pulse',
          )}
        >
          <Waveform state={state} levelRef={levelRef} celebrating={celebrating} className={cn('h-7 w-[72px] shrink-0 overflow-visible', STRANDS)} />
          <span aria-live="polite" className="truncate">
            {statusText(listener)}
          </span>
          {/* A bound field shows the draft itself, so the pill does not repeat it. */}
          {shown ? <span className="text-muted-foreground max-w-64 truncate">{shown}</span> : null}
          <span aria-hidden="true" className="flex shrink-0 items-center gap-1">
            {[0, 1, 2].map((i) => (
              <i
                key={i}
                style={{ animationDelay: `${i * 0.15}s` }}
                className={cn(
                  'bg-current size-1 rounded-full opacity-40',
                  working && 'motion-safe:animate-pulse',
                )}
              />
            ))}
          </span>
          {listener.langLocked ? null : (
            <Button
              type="button"
              variant="secondary"
              size="sm"
              className="h-5.5 rounded-md px-1.5 text-[11px] font-semibold"
              aria-label={t(lang, 'action.switchLang', { lang: langName(lang) })}
              onClick={listener.cycleLang}
            >
              {langShort(lang)}
            </Button>
          )}
        </div>
      ) : null}
      <Button
        {...mic}
        variant={mic['aria-pressed'] ? 'default' : 'outline'}
        size="icon"
        className="relative size-10 rounded-full"
      >
        <span
          ref={ring}
          aria-hidden="true"
          className={cn(
            'ring-ring/60 pointer-events-none absolute inset-0 rounded-full ring-2 transition-transform duration-75',
            state === 'listening' ? 'opacity-100 motion-safe:animate-ping' : 'opacity-0',
          )}
        />
        <svg viewBox="0 0 24 24" aria-hidden="true" className="relative size-[18px]">
          <rect x="9" y="3" width="6" height="11" rx="3" fill="currentColor" />
          <path
            d="M5.5 11.5a6.5 6.5 0 0 0 13 0M12 18v3"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.8"
            strokeLinecap="round"
          />
        </svg>
      </Button>
    </div>
  );
}
