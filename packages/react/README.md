# happybirthdayvalera

> For Valera, on your birthday. May everything you say be heard, and understood, in every language you speak.

A voice-input widget for React: a mic button and a dark pill with an animated three-strand waveform, a status
line, three dots and a language chip. It does real speech-to-text in English, Russian, German and French.
By default it runs Whisper **locally in the browser**. No audio leaves the device.

## Install

```sh
npm install happybirthdayvalera
```

React 18 or 19 is a peer dependency. The package needs no other runtime dependency: transformers.js is
already bundled into the worker file that ships with it.

## Usage

```tsx
import { ListenerPill, MicButton, useListener } from 'happybirthdayvalera';
import 'happybirthdayvalera/style.css';

export function Toolbar() {
  const listener = useListener({ onResult: (text, lang) => console.log(lang, text) });
  return (
    <>
      <MicButton listener={listener} />
      {listener.state !== 'idle' && <ListenerPill listener={listener} />}
    </>
  );
}
```

The first tap asks for the microphone and downloads the model. Later taps start at once.

## Options of `useListener`

| Option | Default | Meaning |
| --- | --- | --- |
| `lang` | detected | Force `'en' \| 'ru' \| 'de' \| 'fr'`. The language chip is hidden while it is set. |
| `persistLang` | `false` | Remember the language picked with the chip in `localStorage`. |
| `engine` | `'whisper'` | `'whisper'`, `'webspeech'` or your own `SpeechEngine`. |
| `model` | `'base'` | Whisper size: `'tiny'`, `'base'` or `'small'`. |
| `onResult(text, lang)` | none | Called once per utterance with the final text. |
| `onInterim(text, lang)` | none | Called with partial text while listening. |
| `interimEveryMs` | `0` | Transcribe the audio so far at this interval. `0` turns it off. |
| `silenceMs` | `1500` | Silence after speech that ends the recording. |
| `easterEgg` | `false` | Celebrate when the text contains a birthday phrase, in any of the four languages. |
| `preload` | `false` | Load the model on mount, not on the first tap. |

The language is the forced `lang`, then the stored choice (with `persistLang`), then the browser language.
It is also the recognition language, and the language of every string in the widget.

The hook returns `state`, `error`, `progress` (0 to 1), `text`, `interim`, `lang`, `setLang`, `cycleLang`,
`start`, `stop`, `toggle`, `reset`, `celebrating` and `levelRef`. It also returns two flags that the pill
needs: `nothingHeard` (the last recording had no speech) and `langLocked` (`lang` is forced).

`levelRef.current` is the smoothed input level from 0 to 1. It changes every frame and never causes a
re-render. Read it in your own `requestAnimationFrame` loop.

Recording stops by itself when you stay silent for `silenceMs` after speaking, or after 30 seconds.
A recording with no speech is not sent to the model: Whisper makes up text from silence.

## Engines

| Engine | Download | Where the audio goes |
| --- | --- | --- |
| `whisper` | model files, once | nowhere; it runs in a Web Worker on your device |
| `webspeech` | none | the browser vendor's service (Chrome and Safari), so it is not local |

`webspeech` fails with the `unsupported` error in browsers that have no `SpeechRecognition`.
You can pass any object that implements `SpeechEngine`:

```ts
interface SpeechEngine {
  load(model: ModelSize, onProgress: (fraction: number) => void): Promise<void>;
  transcribe(pcm: Float32Array, lang: Lang): Promise<string>; // 16 kHz mono Float32
  dispose?(): void;
}
```

### Model sizes

Whisper models from `onnx-community`, quantized to 8 bit. Sizes are approximate downloads.

| `model` | Download | Notes |
| --- | --- | --- |
| `tiny` | about 40 MB | fast, makes more mistakes |
| `base` | about 80 MB | the default; a good balance |
| `small` | about 250 MB | best accuracy, slow on a phone |

The language is forced, so Whisper does not guess it. The task is always `transcribe`, never `translate`.

### The worker

Whisper runs in a module Web Worker. The package creates it with
`new Worker(new URL('./whisper-worker.js', import.meta.url), { type: 'module' })`.
Vite, webpack 5 and Next.js recognise this pattern and emit the worker file for you.

If your setup does not, pass your own:

```ts
import { useListener, whisperEngine } from 'happybirthdayvalera';

const engine = whisperEngine({ workerUrl: '/assets/whisper-worker.js' });         // a URL you serve
// or
const engine2 = whisperEngine({
  createWorker: () => new Worker(new URL('happybirthdayvalera/whisper-worker.js', import.meta.url), { type: 'module' }),
});
useListener({ engine });
```

The worker file is also published as `happybirthdayvalera/whisper-worker.js`. With Vite, add
`optimizeDeps: { exclude: ['happybirthdayvalera'] }` in dev, so the dependency optimizer does not move the
module away from its worker file. The CommonJS build has no worker detection; use `workerUrl` there.

### What "offline" means

- **Audio** is never uploaded when you use `whisper`.
- **Model files** come from the Hugging Face Hub on the first use and are kept in the browser Cache API.
  After that the model loads without the network.
- **The ONNX runtime** (a `.wasm` file and its loader) is fetched from jsDelivr by transformers.js, then the
  browser caches it. For a fully offline app, serve those files yourself and cache the model files with a
  service worker, or host the model files on your own origin.

## Styles

`import 'happybirthdayvalera/style.css'` adds a small stylesheet. Every colour, size, radius, font and
duration is a CSS custom property named `--hbv-<group>-<key>`, for example `--hbv-color-pill-bg`. Override
them on any ancestor:

```css
.toolbar {
  --hbv-color-pill-bg: #102a43;
  --hbv-color-strand-0: #ffd166;
}
```

The markup uses stable class names: `hbv-mic`, `hbv-mic__ring`, `hbv-mic__icon`, `hbv-pill`, `hbv-wave`,
`hbv-wave__strand`, `hbv-pill__status`, `hbv-pill__text`, `hbv-dots` and `hbv-lang`. `data-state` carries the
machine state. `MicButton` and `ListenerPill` accept a `className`.

The package marks CSS files as side effects, so a bundler keeps the import.

## Accessibility

- `MicButton` is a real `<button>` with `aria-pressed` (true while requesting, loading the model and
  listening) and a label that switches between "Start voice input" and "Stop listening".
- The status text is an `aria-live="polite"` region, so progress, errors and "Didn’t catch that" are
  announced.
- The language chip is a button with the label "Language: English. Switch language".
- All labels follow the selected language.
- Focus rings use `:focus-visible` and `--hbv-color-focus-ring`.

### Reduced motion

With `prefers-reduced-motion: reduce`:

- the waveform is drawn once per state, with time frozen and a fixed amplitude, and no animation frame runs;
- the dots and the pulsing ring do not animate;
- the entry fade and the celebration rainbow are off.

## Easter egg

With `easterEgg: true`, a final text that contains "happy birthday", "с днём рождения",
"alles Gute zum Geburtstag", "joyeux anniversaire" (and a few variants), in any language, sets
`celebrating` for 3 seconds. The pill gets `data-celebrate="true"` and shows a rainbow.

## Server rendering

The components render the idle state on the server and are marked `"use client"`, so they work in the Next.js
App Router. The language is detected after hydration.

## Development

```sh
npm run typecheck && npm run lint && npm test   # fast, no network
npm run test:accuracy                           # real recognition of fixtures/, downloads a model once
```

## License

MIT
