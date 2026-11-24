import { ListenerPill, MicButton, useDictation, type ModelSize, type Theme } from 'happybirthdayvalera';
import 'happybirthdayvalera/style.css';
import { EvListenerPill, EvMicButton } from 'happybirthdayvalera/evinvest';
import { useLayoutEffect, useRef, useState, type RefObject } from 'react';
import { VoiceListener } from '../../../registry/voice-listener/voice-listener';
import { Button } from '@/components/ui/button';
import { model as envModel, modelBaseUrl } from './model-base-url';

const MODELS: { value: ModelSize; label: string }[] = [
  { value: 'tiny', label: 'tiny (~40 MB, library default)' },
  { value: 'base', label: 'base (~80 MB)' },
  { value: 'small', label: 'small (~250 MB)' },
];

const THEMES: { value: Theme; label: string }[] = [
  { value: 'default', label: 'Default' },
  { value: 'shadcn', label: 'shadcn/ui' },
  { value: 'evinvest', label: '@evinvest/uikit' },
];

interface BarProps {
  model: ModelSize;
  engine: 'whisper' | 'webspeech';
  target: RefObject<HTMLTextAreaElement | null>;
  onLang: (lang: string) => void;
}

/** One toolbar per theme. Each owns its listener, so switching themes releases the microphone. */
function Bar({ theme, model, engine, target, onLang }: BarProps & { theme: Theme }) {
  const listener = useDictation(target, {
    model,
    engine,
    theme,
    modelBaseUrl,
    easterEgg: true,
    persistLang: true,
    onResult: (_text, lang) => onLang(lang),
  });
  const showPill = listener.pillVisible;

  if (theme === 'evinvest') {
    return (
      <>
        {showPill ? <EvListenerPill listener={listener} /> : null}
        <EvMicButton listener={listener} variant="secondary" size="lg" />
      </>
    );
  }
  return (
    <>
      {showPill ? <ListenerPill listener={listener} /> : null}
      {theme === 'shadcn' ? (
        <MicButton listener={listener} asChild>
          <Button variant="outline" size="icon" className="size-10 rounded-full" />
        </MicButton>
      ) : (
        <MicButton listener={listener} />
      )}
    </>
  );
}

export function App() {
  // `?theme=shadcn` opens a theme directly, which also makes the page easy to check in a headless browser.
  const [theme, setTheme] = useState<Theme>(() => THEMES.find((t) => t.value === new URLSearchParams(location.search).get('theme'))?.value ?? 'default');
  // The attribute sits on <html>, so the theme paints the whole page, not only <main>.
  useLayoutEffect(() => {
    document.documentElement.dataset.demoTheme = theme;
  }, [theme]);
  const [model, setModel] = useState<ModelSize>(envModel ?? 'tiny');
  const [engine, setEngine] = useState<'whisper' | 'webspeech'>('whisper');
  const [lang, setLang] = useState<string | null>(null);
  const [text, setText] = useState('');
  const field = useRef<HTMLTextAreaElement>(null);

  return (
    <main className="demo">
      <h1>happybirthdayvalera</h1>
      <p className="demo__hint">
        Tap the microphone and say something. Try “Happy birthday, Valera”, in any of the four languages.
      </p>

      <div className="demo__themes" role="group" aria-label="Theme">
        {THEMES.map((t) => (
          <button key={t.value} type="button" aria-pressed={theme === t.value} onClick={() => setTheme(t.value)}>
            {t.label}
          </button>
        ))}
      </div>

      <section className="demo__result">
        <label className="demo__field">
          <span>
            Dictation{lang ? <span className="demo__lang">{lang.toUpperCase()}</span> : null}
          </span>
          <textarea
            ref={field}
            rows={5}
            placeholder="Your words appear here, at the caret."
            value={text}
            onChange={(e) => setText(e.target.value)}
          />
        </label>
        {theme === 'shadcn' ? (
          <div className="demo__registry">
            The shadcn registry item, built from your own Button and the shadcn tokens:
            <VoiceListener
              target={field}
              model={model}
              modelBaseUrl={modelBaseUrl}
              engine={engine}
              easterEgg
              onResult={(_t, l) => setLang(l)}
            />
          </div>
        ) : null}
      </section>

      <footer className="demo__bar">
        <label>
          Engine
          <select value={engine} onChange={(e) => setEngine(e.target.value === 'webspeech' ? 'webspeech' : 'whisper')}>
            <option value="whisper">Whisper (local)</option>
            <option value="webspeech">Web Speech (browser)</option>
          </select>
        </label>
        <label>
          Model
          <select
            value={model}
            disabled={engine !== 'whisper'}
            onChange={(e) => setModel(MODELS.find((m) => m.value === e.target.value)?.value ?? 'tiny')}
          >
            {MODELS.map((m) => (
              <option key={m.value} value={m.value}>
                {m.label}
              </option>
            ))}
          </select>
        </label>
        <span className="demo__spacer" />
        <Bar key={theme} theme={theme} model={model} engine={engine} target={field} onLang={setLang} />
      </footer>
    </main>
  );
}
