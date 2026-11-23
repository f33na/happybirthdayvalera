import { ListenerPill, MicButton, useListener, type ModelSize } from 'happybirthdayvalera';
import 'happybirthdayvalera/style.css';
import { useState } from 'react';

const MODELS: { value: ModelSize; label: string }[] = [
  { value: 'tiny', label: 'tiny (~40 MB)' },
  { value: 'base', label: 'base (~80 MB)' },
  { value: 'small', label: 'small (~250 MB)' },
];

export function App() {
  const [model, setModel] = useState<ModelSize>('base');
  const [engine, setEngine] = useState<'whisper' | 'webspeech'>('whisper');
  const [heard, setHeard] = useState<{ text: string; lang: string } | null>(null);

  const listener = useListener({
    model,
    engine,
    easterEgg: true,
    persistLang: true,
    onResult: (text, lang) => setHeard({ text, lang }),
  });

  return (
    <main className="demo">
      <h1>happybirthdayvalera</h1>
      <p className="demo__hint">
        Tap the microphone and say something. Try “Happy birthday, Valera”, in any of the four languages.
      </p>

      <section className="demo__result" aria-live="polite">
        {heard ? (
          <>
            <span className="demo__lang">{heard.lang.toUpperCase()}</span>
            <q>{heard.text}</q>
          </>
        ) : (
          <span className="demo__empty">Your words appear here.</span>
        )}
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
            onChange={(e) => setModel(MODELS.find((m) => m.value === e.target.value)?.value ?? 'base')}
          >
            {MODELS.map((m) => (
              <option key={m.value} value={m.value}>
                {m.label}
              </option>
            ))}
          </select>
        </label>
        <span className="demo__spacer" />
        {listener.state !== 'idle' || listener.nothingHeard ? <ListenerPill listener={listener} /> : null}
        <MicButton listener={listener} />
      </footer>
    </main>
  );
}
