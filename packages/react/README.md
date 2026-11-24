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
      {listener.pillVisible && <ListenerPill listener={listener} />}
    </>
  );
}
```

The first tap asks for the microphone and downloads the model. Later taps start at once.

## Dictation into a text field

`useDictation(ref, options)` takes the same options as `useListener` and returns the same listener, bound to a
`<textarea>` or `<input>`. You write no glue code:

```tsx
import { ListenerPill, MicButton, useDictation } from 'happybirthdayvalera';

export function Composer() {
  const field = useRef<HTMLTextAreaElement>(null);
  const [text, setText] = useState('');
  const listener = useDictation(field);
  return (
    <>
      <textarea ref={field} value={text} onChange={(e) => setText(e.target.value)} />
      <MicButton listener={listener} />
      {listener.pillVisible && <ListenerPill listener={listener} />}
    </>
  );
}
```

- The text goes to the caret, or replaces the selection, at the moment you tap the mic. If the field does not
  have the focus, the text goes to the end. The mic does not take the focus from a bound field.
- While you speak, a draft of the current phrase appears in the field and is replaced in place. After a pause the
  final text of the phrase replaces the draft, and the next phrase continues after it.
- One space goes before the inserted text when it is needed. The first letter becomes a capital at the start of the
  field and after `.`, `!` or `?`. Whisper's punctuation stays as it is.
- One space goes after the inserted text when the next character is not whitespace and not punctuation that
  attaches to the left (`. , ! ? ; : ) ] » ”`).
- If you type outside the draft, the draft is removed and the final text of that phrase goes to your caret. If you
  type inside the draft, the text is yours: the final of that phrase is not inserted, so nothing is repeated.
- Every change fires a native `input` event, so a controlled field (`value` + `onChange`) sees it.
- Cmd/Ctrl+Z removes the last phrase in one step: it is inserted with `document.execCommand('insertText')`, and
  drafts never enter the undo stack. Gap: when the field does not have the focus (for example, a keyboard user
  pressed the mic), the text is set through the value setter, and the browser cannot undo it.
- `Esc` in the field stops listening. The pill does not repeat the draft, because the field shows it.
- `useDictation(null, options)` binds nothing and works like `useListener`.

## Options of `useListener`

| Option | Default | Meaning |
| --- | --- | --- |
| `lang` | detected | Force `'en' \| 'ru' \| 'de' \| 'fr'`. The language chip is hidden while it is set. |
| `persistLang` | `false` | Remember the language picked with the chip in `localStorage`. |
| `engine` | `'whisper'` | `'whisper'`, `'webspeech'` or your own `SpeechEngine`. |
| `model` | `'tiny'` | Whisper size: `'tiny'`, `'base'` or `'small'`. |
| `mode` | `'continuous'` | `'continuous'`: listen until stopped and split the speech at pauses. `'utterance'`: one phrase, then stop. |
| `onResult(text, lang)` | none | Called with the final text of every phrase (segment). In `utterance` mode, once. |
| `onInterim(text, lang)` | none | Called with every draft of the current phrase. |
| `interimEveryMs` | `1000` | Make a draft of the current phrase at this interval. `0` turns drafts off. |
| `segmentSilenceMs` | `700` | `continuous`: a pause this long after speech ends a phrase. |
| `autoStopMs` | `8000` | `continuous`: no speech for this long stops listening. |
| `segmentPadBeforeMs` / `segmentPadAfterMs` | `200` / `300` | Audio kept before and after the speech of a phrase. |
| `maxSegmentMs` | `28000` | A longer phrase is cut here; Whisper sees at most 30 s. |
| `silenceMs` | `1500` | `utterance`: silence after speech that ends the recording. |
| `holdMs` | `300` | A press on the mic longer than this is hold-to-talk. |
| `easterEgg` | `false` | Celebrate when the text contains a birthday phrase, in any of the four languages. |
| `preload` | `false` | Load the model on mount, not on the first tap. |
| `theme` | `'default'` | `'default'`, `'shadcn'` or `'evinvest'`. See [Theming](#theming). |
| `modelBaseUrl` | Hub | Base URL of your own copy of the model files and the onnxruntime files. See [Self-hosted models](#self-hosted-models). |
| `wasmPaths` | CDN | Base URL of the onnxruntime `.wasm` files (default Whisper engine). See [Fully offline](#fully-offline). |
| `device` | `'auto'` | `'auto'`, `'webgpu'` or `'wasm'`: where Whisper runs. See [Speed](#speed). |

The language is the forced `lang`, then the stored choice (with `persistLang`), then the browser language.
It is also the recognition language, and the language of every string in the widget.

The hook returns `state`, `error`, `progress` (0 to 1), `text` (the committed text of the session, phrases
joined with one space), `interim` (the draft of the current phrase), `lang`, `setLang`, `cycleLang`, `start`,
`stop`, `toggle`, `reset`, `celebrating`, `levelRef` and `theme`. It also returns three values that the pill
needs: `nothingHeard` (the last session had no speech), `langLocked` (`lang` is forced) and `pillVisible`.
For hosts that draw their own mic there are `pressStart`, `pressEnd`, `click(fromPointer)`, `holding` and
`bound`; `micButtonProps` wires them for you.

Mount the pill on `pillVisible`, not on `state !== 'idle'`. The easter egg is shown while the state is `idle`,
so a pill that hides in `idle` never shows it. `progress` is a fraction, or `null` while the size of a model file
is unknown; the status then reads "Loading model…" and never shows a made-up percent.
`useListener` renders nothing, so you can build your own UI on it.

`levelRef.current` is the smoothed input level from 0 to 1. It changes every frame and never causes a
re-render. Read it in your own `requestAnimationFrame` loop.

In the default `continuous` mode a tap starts listening, and the microphone stays on until a second tap, `Esc`, or
`autoStopMs` without speech. Each pause of `segmentSilenceMs` ends a phrase. The phrase goes to the model in the
background, and its text is committed in order while you go on speaking. A tap that stops waits until every phrase
is committed. In `utterance` mode, recording stops by itself after `silenceMs` of silence after speech, or after
30 seconds.

Press and hold the mic longer than `holdMs` to talk only while you hold it: release stops. A shorter press is a tap.
While held, the mic has `data-hold="true"`. Space and Enter always toggle.

Audio with no speech is not sent to the model: Whisper makes up text from silence. Only one model call runs at a
time, and a final phrase always goes before a draft. A draft that cannot start because the model is busy is
skipped, so a slow model makes fewer drafts instead of a queue.

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
| `tiny` | about 40 MB | the default; fast, makes more mistakes |
| `base` | about 80 MB | a good balance |
| `small` | about 250 MB | best accuracy, slow on a phone |

The language is forced, so Whisper does not guess it. The task is always `transcribe`, never `translate`.

### Speed

One Whisper call costs about the same for 1 s and for 6 s of audio, because Whisper pads its input to 30 s.
Measured in Chrome on an Apple M2, per call:

| Setup | `tiny` | `base` |
| --- | --- | --- |
| wasm, one thread (no headers) | 1.2–1.4 s | 2.8–3.0 s |
| wasm, 4 threads (cross-origin isolated) | 0.45–0.75 s | 0.8–1.25 s |
| WebGPU (`base` with `modelBaseUrl`) | — | 0.3–0.4 s |

**Send two headers on your page** to get the threads:

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

The worker then runs onnxruntime on `min(4, cores)` threads. Without the headers it runs on one thread: it still
works, only slower. With `require-corp`, every cross-origin file of your page needs CORS or a
`Cross-Origin-Resource-Policy` header. The Hugging Face Hub and jsDelivr send CORS. If an embed of yours cannot,
use `Cross-Origin-Embedder-Policy: credentialless` (Chrome and Firefox). In Vite set `server.headers` and
`preview.headers`; in Next.js use `headers()` in `next.config`.

**WebGPU.** With `device: 'auto'` (the default), `base` runs on WebGPU when the browser has an adapter and you set
`modelBaseUrl`. WebGPU needs other files than wasm: `onnx/encoder_model.onnx` (fp32) and
`onnx/decoder_model_merged_fp16.onnx`, 187 MB for `base` instead of 77 MB, so `auto` never pulls them from the Hub.
`tiny` stays on wasm threads: it is fast there, and on WebGPU it got a French test phrase wrong. If WebGPU fails to
load, `auto` falls back to wasm. `device: 'webgpu'` forces WebGPU for any model, from the Hub too; `'wasm'` never
uses it.

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
- **The ONNX runtime** (a `.wasm` file and its loader) is fetched from jsDelivr by default, then the browser
  caches it.

### Fully offline

To run with no network at all after the first visit:

1. Copy `ort-wasm-simd-threaded.jsep.mjs` and `ort-wasm-simd-threaded.jsep.wasm` from
   `node_modules/@huggingface/transformers/dist/` (the files of the pinned transformers.js version) to a folder
   that you serve, for example `/ort/`.
2. Pass the folder: `useListener({ wasmPaths: '/ort/' })`, or `whisperEngine({ wasmPaths: '/ort/' })`.
3. Cache the model files. The first load stores them in the browser Cache API. For an app that must work
   before the first load succeeds, add a service worker that caches the Hugging Face model URLs, or host the
   model files yourself.

### Self-hosted models

By default the model files come from the Hugging Face Hub and the onnxruntime files from jsDelivr. To serve them
from your own host or CDN, pass `modelBaseUrl`: `useListener({ modelBaseUrl: 'https://cdn.example.com/hbv/' })`
or `whisperEngine({ modelBaseUrl })`. A missing trailing `/` is added. The host needs CORS and a
`content-length` header on `HEAD` and `GET`. Without `content-length` the progress stays unknown.

```text
<base>onnx-community/whisper-tiny/config.json
<base>onnx-community/whisper-tiny/onnx/encoder_model_quantized.onnx
<base>ort/ort-wasm-simd-threaded.jsep.mjs     (the onnxruntime files; `wasmPaths` wins if you set it)
```

In the repository, `just fetch-models` (`base` by default, with its WebGPU files) and `just serve-models` build and
serve this layout for local work. The server sends `Cross-Origin-Resource-Policy: cross-origin`, so cross-origin-isolated
pages can load from it.

`import 'happybirthdayvalera/style.css'` adds one small stylesheet. Every colour, size, radius, font and
duration is a CSS custom property named `--hbv-<group>-<key>`, for example `--hbv-color-pill-bg`. The
defaults are declared on `:where(.hbv-mic, .hbv-pill)`, with zero specificity, so any host class or scope wins:

```css
.toolbar {
  --hbv-color-pill-bg: #102a43;
  --hbv-color-strand-0: #ffd166;
}
```

The markup uses stable class names: `hbv-mic`, `hbv-mic__ring`, `hbv-mic__icon`, `hbv-pill`, `hbv-wave`,
`hbv-wave__strand`, `hbv-pill__status`, `hbv-pill__text`, `hbv-dots` and `hbv-lang`. `data-state` carries the
machine state. The package marks CSS files as side effects, so a bundler keeps the import.

## Theming

The widget can look native in a host design system. Use the first level that is enough.

### 1. A preset

```tsx
const listener = useListener({ theme: 'shadcn' }); // 'default' | 'shadcn' | 'evinvest'
```

The theme is rendered as `data-hbv-theme` on the mic button and the pill. A preset only maps the `--hbv-*`
tokens to your design system's CSS variables, and the variables resolve in the nearest host scope (`.dark`,
`.light`, `[data-brand]`). It needs no Tailwind and no extra package. You can override one component with
`<MicButton theme="..." />`.

- `shadcn` targets shadcn/ui on Tailwind v4 (oklch tokens used as raw `var(--popover)`). For shadcn v3, which
  stores HSL triplets, override the tokens yourself, for example `--hbv-color-pill-bg: hsl(var(--popover))`.
- `evinvest` targets `@evinvest/uikit` 0.36 or newer.

### 2. Classes, your own button and your own parts

Both components take `className` and a `classNames` map. Your classes come after the contract classes.

```tsx
<MicButton listener={listener} classNames={{ root: 'shadow-lg', ring: '...', icon: '...' }} />
<ListenerPill listener={listener} classNames={{ root, wave, strand, status, text, dots, lang }} />
```

`MicButton` takes `asChild`. It renders your element instead of its own `<button>`, merges `className`,
composes `onClick`, forwards `ref` and adds all the aria and data attributes:

```tsx
<MicButton listener={listener} theme="shadcn" asChild>
  <Button variant="outline" size="icon" />
</MicButton>
```

`ListenerPill` takes `components={{ LangButton }}` for the language chip. Your component gets button props and
`children`, and must spread them on one element.

For a fully custom UI, use the headless parts: `useListener`, `Waveform`, `wavePath(i, t, A)`,
`t(lang, key, vars)`, `statusText(listener)`, `langName`, `langShort` and `micButtonProps(listener)`. The last one
returns the `aria-*`, `data-*` and `type` props and the click, pointer, mouse and key handlers (tap, hold to talk, `Esc`)
for any element.

### 3. Native adapters

**`@evinvest/uikit`**: `happybirthdayvalera/evinvest` is a separate entry. The main entry never imports it, and
`@evinvest/uikit` (`>=0.36 <1`) is an optional peer dependency.

```tsx
import { EvListenerPill, EvMicButton } from 'happybirthdayvalera/evinvest';

<EvListenerPill listener={listener} />
<EvMicButton listener={listener} variant="secondary" size="lg" />
```

`EvMicButton` is a native `<button>` with the kit's `buttonVariants` classes, so `aria-pressed` and
`aria-label` stay. The language chip is the kit's `Badge` over a `<button>`. Both use the `evinvest` theme.
The adapter adds no Tailwind classes of its own, because Tailwind in your app cannot scan this package.
It needs the kit's own setup (Tailwind v4 and `@evinvest/uikit/styles/tokens.css`). The kit loads its shell
components with the barrel import, so its peers `@evinvest/mfe` and `@evinvest/keymap` must be installed, as in
every app that uses the kit.

**shadcn/ui**: a registry item copies a component into your app. It uses your `Button`, Tailwind classes on the
shadcn tokens, and the headless hook and waveform:

```sh
npx shadcn add <url of registry/public/r/voice-listener.json>
```

The URL works once the repository is public. The item imports `@/components/ui/button` and `@/lib/utils`, and
adds the `happybirthdayvalera` dependency. Pass `target={ref}` to dictate into a `<textarea>` or `<input>`
(see [Dictation into a text field](#dictation-into-a-text-field)).

Known limit: the base stylesheet is not in a cascade layer, so for the properties that it sets (background,
size, radius of the mic) it wins over Tailwind utilities, which live in `@layer utilities`. The presets map the
tokens so that the result matches the host. To restyle a part, set the `--hbv-*` token or a host class.

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
