import { t, errorKey, langName, langShort } from '../core/i18n';
import type { Listener } from './useListener';
import { Waveform } from './Waveform';

export interface ListenerPillProps {
  listener: Listener;
  className?: string;
}

function statusText(listener: Listener): string {
  const { state, lang, error, progress, nothingHeard } = listener;
  switch (state) {
    case 'error':
      return t(lang, error ? errorKey(error) : 'error.transcribeFailed');
    case 'requesting':
      return t(lang, 'status.requesting');
    case 'loading-model':
      return t(lang, 'status.loadingModel', { percent: Math.round(progress * 100) });
    case 'listening':
      return t(lang, 'status.listening');
    case 'transcribing':
      return t(lang, 'status.transcribing');
    case 'idle':
      return t(lang, nothingHeard ? 'status.nothingHeard' : 'status.idle');
  }
}

export function ListenerPill({ listener, className }: ListenerPillProps) {
  const { state, lang, levelRef, celebrating, langLocked, interim, text } = listener;
  const shown = state === 'listening' || state === 'transcribing' ? interim : text;
  return (
    <div
      className={className ? `hbv-pill ${className}` : 'hbv-pill'}
      role="group"
      aria-label={t(lang, 'label.widget')}
      data-state={state}
      data-celebrate={celebrating ? 'true' : 'false'}
    >
      <Waveform state={state} levelRef={levelRef} />
      <span className="hbv-pill__status" aria-live="polite">
        {statusText(listener)}
      </span>
      {shown ? <span className="hbv-pill__text">{shown}</span> : null}
      <span className="hbv-dots" aria-hidden="true">
        <i />
        <i />
        <i />
      </span>
      {langLocked ? null : (
        <button
          type="button"
          className="hbv-lang"
          aria-label={t(lang, 'action.switchLang', { lang: langName(lang) })}
          onClick={listener.cycleLang}
        >
          {langShort(lang)}
        </button>
      )}
    </div>
  );
}
