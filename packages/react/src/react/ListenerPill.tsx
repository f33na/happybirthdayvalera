import type { ButtonHTMLAttributes, ComponentType } from 'react';
import { t, errorKey, langName, langShort } from '../core/i18n';
import { cx, type PillClassNames, type Theme } from './slots';
import type { Listener } from './useListener';
import { Waveform } from './Waveform';

/** Props that the pill passes to the language button. A host component must spread them on one element. */
export type LangButtonProps = ButtonHTMLAttributes<HTMLButtonElement>;

export interface ListenerPillProps {
  listener: Listener;
  /** Overrides `listener.theme` for this component. */
  theme?: Theme;
  className?: string;
  /** Classes for the parts, added after the contract class names. */
  classNames?: PillClassNames;
  /** Replace parts with your own components, for example the language chip. */
  components?: { LangButton?: ComponentType<LangButtonProps> };
}

/** The status line of the pill, in the listener's language. For hosts that draw their own pill. */
export function statusText(listener: Listener): string {
  const { state, lang, error, progress, nothingHeard } = listener;
  switch (state) {
    case 'error':
      return t(lang, error ? errorKey(error) : 'error.transcribeFailed');
    case 'requesting':
      return t(lang, 'status.requesting');
    case 'loading-model':
      return progress === null
        ? t(lang, 'status.loadingModelUnknown')
        : t(lang, 'status.loadingModel', { percent: Math.round(progress * 100) });
    case 'listening':
      return t(lang, 'status.listening');
    case 'transcribing':
      return t(lang, 'status.transcribing');
    case 'idle':
      return t(lang, nothingHeard ? 'status.nothingHeard' : 'status.idle');
  }
}

export function ListenerPill({ listener, theme, className, classNames, components }: ListenerPillProps) {
  const { state, lang, levelRef, celebrating, langLocked, interim, text } = listener;
  const shown = state === 'listening' || state === 'transcribing' ? interim : text;
  const LangButton = components?.LangButton ?? 'button';
  return (
    <div
      className={cx('hbv-pill', className, classNames?.root)}
      role="group"
      aria-label={t(lang, 'label.widget')}
      data-state={state}
      data-celebrate={celebrating ? 'true' : 'false'}
      data-hbv-theme={theme ?? listener.theme}
    >
      <Waveform state={state} levelRef={levelRef} celebrating={celebrating} className={classNames?.wave} strandClassName={classNames?.strand} />
      <span className={cx('hbv-pill__status', classNames?.status)} aria-live="polite">
        {statusText(listener)}
      </span>
      {shown ? <span className={cx('hbv-pill__text', classNames?.text)}>{shown}</span> : null}
      <span className={cx('hbv-dots', classNames?.dots)} aria-hidden="true">
        <i />
        <i />
        <i />
      </span>
      {langLocked ? null : (
        <LangButton
          type="button"
          className={cx('hbv-lang', classNames?.lang)}
          aria-label={t(lang, 'action.switchLang', { lang: langName(lang) })}
          onClick={listener.cycleLang}
        >
          {langShort(lang)}
        </LangButton>
      )}
    </div>
  );
}
