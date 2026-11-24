# Shared contract

Both libraries, `packages/react` (npm) and `crates/happybirthdayvalera` (crates.io), follow this file. When a
library needs a change here, change this file in the same commit, and change the other library too.

The JSON files next to this one are the data side of the contract:

| File | Content |
| --- | --- |
| `locales.json` | Languages, UI strings, birthday phrases |
| `design-tokens.json` | Colours, sizes, radii, fonts, motion, waveform parameters |
| `behavior.json` | VAD, timing and model defaults |
| `machine.json` | States, events, error codes, transitions |
| `styles/listener.css` | Hand-written base CSS that uses only `var(--hbv-*)`; the generator adds the tokens |

## Clean-room rule

The look is inspired by a voice pill seen on a third-party site. Do not download, copy or imitate its code,
assets, colours or copy. Everything here is original.

## Languages

Supported codes: `en`, `ru`, `de`, `fr`. The fallback is `en`.

`resolvePreferredLang(list)` / `resolve_preferred_lang(list)`: walk the list in order. For each tag, take the
primary subtag (the part before the first `-` or `_`), lowercase it, and return it if it is supported. If no
tag matches, return `en`. Examples: `["ru-RU"]` → `ru`, `["pt-BR", "de"]` → `de`, `["pt-BR"]` → `en`,
`[]` → `en`, `["FR_ca"]` → `fr`.

Sources of the list: the web uses `navigator.languages`, then `navigator.language`. Desktop uses
`sys_locale::get_locales()`.

Priority of the active language: a forced `lang` prop, then the value in storage (web, only with
`persistLang`), then the resolved locale. While `lang` is forced, the language chip is hidden.

The UI language is the selected recognition language. The chip cycles in `behavior.json` → `langCycle` order
and shows `short` from `locales.json`.

Placeholders in strings: `{percent}` is an integer 0–100. `{lang}` is the language `name`.

## State machine

See `machine.json`. Notes:

- `MIC_READY` always goes to `loading-model`. If the model is already loaded, the controller sends
  `MODEL_READY` at once.
- Audio is kept only while the state is `listening`.
- `STOP` in `requesting` or `loading-model` cancels and releases the microphone.
- `RESULT` with empty text goes to `idle`, and the UI shows `status.nothingHeard` until the next `START`.
- After a non-empty `RESULT`, the UI shows `status.idle`, and `onResult(text, lang)` fires once.
- `progress` is a fraction 0..1, or unknown (`null` in TS, `None` in Rust). The status is
  `status.loadingModel` with `{percent}` when it is known, and `status.loadingModelUnknown` when it is not.
  Do not fake a percentage: a response without a readable size (CDN without `content-length`, a Cache API
  hit) makes the progress unknown until every file size is known.
- `pillVisible` / `pill_visible` is `state !== 'idle' || nothingHeard || celebrating`. Hosts mount the pill
  on it, and the README examples use it. A host that hides the pill in `idle` would never show the easter
  egg.
- Leaving `listening` releases the microphone at once, before transcription ends.

Error code to string key: `mic-denied` → `error.micDenied`, `mic-unavailable` → `error.micUnavailable`,
`unsupported` → `error.unsupported`, `model-failed` → `error.modelFailed`, `transcribe-failed` →
`error.transcribeFailed`.

## Audio and VAD

- The capture is mono Float32 PCM at 16 000 Hz. Resample on the client: an AudioWorklet on the web, a
  linear or sinc resampler after cpal on desktop.
- The level is the RMS of each block of about 20 ms.
- Speech starts when the RMS stays at or above `vadThresholdRms` for `minSpeechMs`.
- `SILENCE` fires when, after speech started, the RMS stays below the threshold for `silenceMs`. There is no
  auto-stop before speech starts.
- In `utterance` mode, at `maxDurationMs` the controller sends `STOP`.
- In `utterance` mode, `SILENCE` ends the utterance as above. Drafts and continuous mode are in "Live dictation".


## Live dictation

The default `mode` is `continuous`. The old behaviour, one utterance and then stop, stays as
`mode: 'utterance'` (`Mode::Utterance` in Rust). All new numbers are in `behavior.json` and are overridable.

### Continuous mode

- A tap on the mic starts listening. The microphone stays on until `STOP`: a second tap, `Esc` on the mic
  or in a bound text field, or the auto-stop.
- Pauses split the speech into segments. A segment ends when, after speech started in it, the RMS stays
  below the threshold for `segmentSilenceMs`. A segment is also cut at `maxSegmentMs`, because Whisper
  sees at most 30 s.
- The audio of a finished segment goes to the engine with `segmentPadBeforeMs` of audio before the speech
  start and `segmentPadAfterMs` after the speech end (clamped to the captured audio). The machine stays in
  `listening` while segments are transcribed in the background.
- Segment results are committed strictly in segment order. Each non-empty segment result fires
  `onResult(text, lang)` once. An empty segment result fires nothing.
- `text` / `text()` holds the committed text of the session so far, segments joined with one space.
- Auto-stop: no speech for `autoStopMs` (counted from the start of listening or from the end of the last
  speech) sends `STOP`. `maxDurationMs` does not apply to the session in this mode.
- `STOP` in `listening`: the open segment is closed if it has speech, and the machine goes to
  `transcribing` until every queued segment is committed. Then `RESULT` with the session text goes to
  `idle`. If the session committed no text, `RESULT` is empty and `nothingHeard` holds as before.
  The microphone is released when `listening` ends, as before.
- The easter egg checks every committed segment.
- `machine.json` does not change. In continuous mode the controller does not send `SILENCE` to the machine.

### Drafts (interim text)

- While a segment is open and has speech, the controller transcribes **only the audio of that segment** (with
  `segmentPadBeforeMs`) every `interimEveryMs` (default 1000; 0 turns drafts off). The cost per draft stays
  bounded however long the session is.
- One engine call runs at a time. A final segment call always goes before a draft. A draft tick is skipped
  while any engine call runs or a final call waits.
- `interim` holds the draft of the open segment. It is cleared when that segment is committed or when the
  segment turns out empty. A draft that comes back after its segment has closed still shows until that segment
  is committed: the final call of the segment cannot start before the draft call ends, so with a slow model this
  late draft is often the only one. `onInterim(text, lang)` fires for every non-empty draft.
- In `utterance` mode drafts work the same way over the single utterance.

### Hold to talk

- Pointer down on the mic and hold longer than `holdMs`: the widget listens while the pointer is down. Pointer
  up sends `STOP`. A press shorter than `holdMs` is a tap and toggles as before. Space and Enter toggle.
- While held, the mic has `data-hold="true"`.
- If the pointer goes down while the state is not `idle`, `nothingHeard` or `error`, it is a plain tap (stop).

### Dictation into a text field

A helper binds a listener to a `<textarea>` or `<input>`, so the host writes no glue code:

- React: `useDictation(ref, options)` returns the same `Listener` as `useListener(options)` and binds it to
  `ref.current`.
- Dioxus: `use_dictation(target_id, config)` returns the same `Listener` and binds it to the element with that
  `id` (web: through `web-sys`; desktop: through `document::eval`).

Binding rules:

- The insertion point is the caret (or the selection, which is replaced) at the moment a session starts. If
  the field is not focused, it is the end of the text.
- The draft is shown **in the field** at the insertion point as plain text, and it is replaced in place on every
  draft and by the final text of the segment. The next segment continues right after the committed text.
- Spacing: add one space before the inserted text when the character before it is not whitespace and not the
  start of the field. Add one space after the inserted text when the character right after it is not whitespace,
  not the end of the field and not punctuation that attaches to the left: `.` `,` `!` `?` `;` `:` `)` `]` `»` `”`.
  The draft is shown with the same spaces, and the next segment continues after the trailing space. Capitalize the first letter when it is at the start of the field or after `.`, `!`
  or `?` and whitespace. Do not change Whisper's own punctuation.
- If the user edits outside the draft, the draft is removed, later drafts of that segment stay hidden, and its
  final text goes to the caret. If the user edits inside the draft, the final of that segment is not inserted, so
  the text is not repeated. Committed segments after that go to the current caret.
- Every change fires a native `input` event so controlled inputs (React `value` + `onChange`, Dioxus
  `oninput`) see it. React uses the native value setter of the element prototype for this.
- Undo: commit a final segment with `document.execCommand('insertText')` while the field is focused, so the
  browser's own Cmd/Ctrl+Z removes it in one step. Drafts must not fill the undo stack. If a browser cannot do
  this reliably, fall back to the value setter and document the gap.
- `Esc` in the bound field while listening sends `STOP`.
- The pill keeps the wave, the status and the language chip. When a field is bound, the pill does not repeat
  the draft text in `.hbv-pill__text`.

### Model for self-hosted setups

`just fetch-models` fetches `base` by default (`just fetch-models tiny` still works). The stands and demos read
`HBV_MODEL` (`tiny` / `base` / `small`). When `HBV_MODEL_BASE_URL` is set and `HBV_MODEL` is not, they use
`base`. The library default stays `tiny`, because without self-hosting it downloads from the Hub.

## Engine

```ts
type Lang = 'en' | 'ru' | 'de' | 'fr';
type ModelSize = 'tiny' | 'base' | 'small';
interface SpeechEngine {
  load(model: ModelSize, onProgress: (fraction: number) => void): Promise<void>;
  transcribe(pcm: Float32Array, lang: Lang): Promise<string>;
  begin?(lang: Lang): void; // called on entering `listening`; for engines that capture audio themselves
  dispose?(): void;
}
```

An engine that captures its own audio, such as the Web Speech API, starts in `begin` and ignores `pcm` in
`transcribe`. `MODEL_FAILED` may carry an error code, for example `unsupported`; without one it means
`model-failed`. If the VAD never saw speech, or the audio is under 0.3 s, the controller does not call the
engine and sends an empty `RESULT`. This stops Whisper from inventing text from silence.

Rust has the same trait with async methods. `begin` is optional there too (a default no-op). Whisper always gets the language as a forced token, with task
`transcribe`, not `translate`. The engine trims the result.

Model ids are in `behavior.json` → `models`. Desktop checks the SHA-256 of a downloaded `ggml-*.bin` against
pinned values in code. It writes to a `.part` file and renames it after the check.

### Web worker protocol

The default web engine runs Whisper (transformers.js) in a module Web Worker. The same worker file serves the
npm package and the Dioxus web build (`crates/happybirthdayvalera/assets/whisper-worker.js`). Messages:

Main → worker:

```ts
{ type: 'load', model: 'onnx-community/whisper-tiny', wasmPaths?: string, modelBaseUrl?: string,
  device?: 'auto' | 'webgpu' | 'wasm' }
// wasmPaths: onnxruntime wasm base URL; default is the CDN.
// modelBaseUrl: self-hosted models, ends with "/" (see "Self-hosted models")
// device: where Whisper runs, default 'auto' (see "Speed")
{ type: 'transcribe', id: number, pcm: Float32Array, lang: Lang }   // pcm buffer is transferred
```

Worker → main:

```ts
{ type: 'progress', fraction?: number }         // 0..1 over all model files; absent while any size is unknown
{ type: 'ready', device?: 'webgpu' | 'wasm' } // the backend that actually runs
{ type: 'result', id: number, text: string }
{ type: 'error', id?: number, message: string } // id is absent for a load error
```

A second `load` with the same model and backend answers `ready` at once.

### Speed

Whisper pads every input to 30 s, so one call costs about the same for 1 s and for 6 s of audio. Two things cut it:

- **Threads.** onnxruntime-web runs on several threads only on a cross-origin-isolated page: the response headers
  `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` (or `credentialless`).
  The worker then sets `numThreads = min(4, navigator.hardwareConcurrency)`; more threads were slower on an 8-core
  M2. Without isolation it runs on one thread, as before. Every cross-origin file must then allow it with CORS or
  `Cross-Origin-Resource-Policy`; `just serve-models` sends `Cross-Origin-Resource-Policy: cross-origin`. The React
  stands and the demo send both headers in `vite dev` and `vite preview`.
- **WebGPU.** `device` is `'auto'` (default), `'webgpu'` or `'wasm'` (`device` in `useListener` and `whisperEngine`).
  WebGPU uses `encoder_model: 'fp32', decoder_model_merged: 'fp16'` (`onnx/encoder_model.onnx`,
  `onnx/decoder_model_merged_fp16.onnx`); wasm keeps q8. The q8 files give garbage on WebGPU, and an fp16 encoder
  breaks `base`. `'auto'` picks WebGPU only when the browser gives an adapter, `modelBaseUrl` is set (the WebGPU
  files are about 2.4 times the q8 download) and the model is `base`. `tiny` stays on wasm: on WebGPU it misses a
  French fixture keyword. `small` is not measured. If WebGPU fails to load under `'auto'`, the worker falls back
  to wasm. `'webgpu'` uses WebGPU for any model and source, and fails loudly; without an adapter every choice runs
  on wasm.
- `just fetch-models base` also fetches the WebGPU pair of `base`.

### Self-hosted models

The default is the Hugging Face Hub, jsDelivr and `behavior.json` → `models.desktop.baseUrl`. An optional base URL
replaces all three. It is `modelBaseUrl` in TypeScript (`useListener`, `whisperEngine`) and
`model_base_url: Option<String>` in `ListenerConfig`. A missing trailing `/` is added. One root serves web and desktop:

| Path | Content |
| --- | --- |
| `<base><hf model id>/…` | transformers.js files, for example `<base>onnx-community/whisper-tiny/onnx/encoder_model_quantized.onnx` |
| `<base>ort/` | the onnxruntime-web `.wasm` and `.mjs` files from `@huggingface/transformers/dist` |
| `<base>ggml-<size>.bin` | the desktop whisper.cpp files, the flat layout of `models.desktop.baseUrl` |

- With `modelBaseUrl` the worker sets `env.allowLocalModels = true`, `env.localModelPath = modelBaseUrl` and
  `env.allowRemoteModels = false`. Without it, `allowLocalModels = false` and the Hub is used.
- With `modelBaseUrl` and no `wasmPaths`, the worker uses `<base>ort/` as `wasmPaths`. An explicit `wasmPaths` wins.
- Progress sizes come from `HEAD` (`content-length`) of the expected model files under `<base><model id>/`, not from
  the Hub API. A failed `HEAD` keeps the progress unknown: no fake percentage.
- On desktop the base replaces `models.desktop.baseUrl` for the download only. The checksum check and the on-disk cache do not change.
- The default model is `tiny` (`models.default`): about 41 MB at q8 (encoder 10.1 MB, decoder 30.7 MB). `base` and `small` stay selectable.
- Dev tooling: `just fetch-models [size]` fills `models/hosted/`, and `just serve-models` serves it on port 5299 with CORS.
  Stands and demos opt in with `HBV_MODEL_BASE_URL` and pick a size with `HBV_MODEL` (Vite: `VITE_HBV_MODEL_BASE_URL`
  and `VITE_HBV_MODEL`; Dioxus: `option_env!`).

## Public API

TypeScript:

```tsx
const listener = useListener({ lang?, persistLang?, engine?, model?, mode?, onResult?, onInterim?,
                               interimEveryMs?, silenceMs?, segmentSilenceMs?, autoStopMs?,
                               segmentPadBeforeMs?, segmentPadAfterMs?, maxSegmentMs?, holdMs?,
                               easterEgg?, preload?, theme? });
const listener = useDictation(ref, options); // the same listener, bound to ref.current; ref = null binds nothing
// listener: { state, error, progress, text, interim, lang, setLang, cycleLang,
//             start, stop, toggle, reset, celebrating, levelRef, theme,
//             nothingHeard, langLocked, pillVisible,
//             pressStart, pressEnd, click(fromPointer), holding, bound }  (progress: number | null)
// nothingHeard: the last RESULT was empty, until the next START. langLocked: `lang` is forced; hide the chip.
<MicButton listener={listener} theme? className? classNames? asChild? />
<ListenerPill listener={listener} theme? className? classNames? components? />
```

Rust (Dioxus 0.7):

```rust
let listener = use_listener(ListenerConfig { lang: None, model: ModelSize::Base, ..Default::default() });
rsx! { MicButton { listener } ListenerPill { listener } }
```

`Listener` is a `Copy` handle with signals. `ListenerConfig` has `theme: Theme`, and the components take
optional `theme`, `class` and `class_names` props (see Theming). Cargo features: `web`, `desktop`, and
`ev_uikit` (an adapter that combines with either of them). The crate does not enable a
Dioxus renderer itself; the app does.

`levelRef` (TS) and the level signal (Rust) carry the smoothed level 0..1. The TS waveform reads it in
`requestAnimationFrame` and writes the SVG `d` attribute through a ref; it does not re-render React per frame.

## DOM and class names

Both libraries render the same markup, so one stylesheet serves both. `data-state` holds the machine state.

```html
<button type="button" class="hbv-mic" data-state="listening" aria-pressed="true"
        aria-label="Stop listening" style="--hbv-level: 0.42">
  <span class="hbv-mic__ring" aria-hidden="true"></span>
  <svg class="hbv-mic__icon" viewBox="0 0 24 24" aria-hidden="true">…</svg>
</button>

<div class="hbv-pill" role="group" aria-label="Voice input" data-state="listening" data-celebrate="false">
  <svg class="hbv-wave" viewBox="0 0 72 28" aria-hidden="true">
    <path class="hbv-wave__strand" data-i="0" d="…"/>
    <path class="hbv-wave__strand" data-i="1" d="…"/>
    <path class="hbv-wave__strand" data-i="2" d="…"/>
  </svg>
  <span class="hbv-pill__status" aria-live="polite">Listening</span>
  <span class="hbv-dots" aria-hidden="true"><i></i><i></i><i></i></span>
  <button type="button" class="hbv-lang" aria-label="Language: English. Switch language">EN</button>
</div>
```

- `aria-pressed` is `true` in `requesting`, `loading-model` and `listening`.
- `data-hold` on the mic is `"true"` while the pointer holds it longer than `holdMs`, else `"false"`.
- `aria-label` of the mic button is `action.stop` while `aria-pressed` is `true`, else `action.start`.
- In `error`, the status shows the error string, and the pill gets `data-state="error"`.
- The dots animate in `listening` and `transcribing` only.
- The pill has an optional `.hbv-pill__text` span after the status for interim or final text.
- `prefers-reduced-motion: reduce` stops the dots, the ring and the waveform time. See below.

## Waveform math

Inputs: the `wave` tokens, the time `t` in seconds since mount, and the smoothed level `L` in 0..1.

Level smoothing, once per animation frame, with `target = clamp(rms * levelGain, 0, 1)`:

```
L += (target - L) * (target > L ? attack : release)
```

Amplitude: `A = idleAmp + (1 - idleAmp) * L` while `listening`; `A = restAmp` in every other state. With
reduced motion: `t = 0` and `A = reducedMotionAmp`, drawn once per state change.

For each strand `i` and each sample `k = 0..points-1`, with `x = k / (points - 1)`, `W = viewBoxWidth`,
`H = viewBoxHeight`:

```
env = sin(π·x)²
y   = H/2 + (H/2 − strokePad) · env · amp_i · A · sin(2π·freq_i·x + phase_i + speed_i·t)
```

The path is `M x0 y0 L x1 y1 …` with `x_k = x·W`. Round every coordinate to 2 decimals. Both libraries have a
unit test that checks all cases in `shared/wave-fixture.json` (three strands at `t = 0.5`, `L = 0.5`). The
reference generator is `scripts/wave-fixture.mjs`; run it again after a change to the `wave` tokens.

Each strand gets `stroke: var(--hbv-color-strand-<i>)`, `stroke-width: strokeWidth`, `opacity: opacity_i`,
`fill: none`, and round caps and joins.

## Theming and design-system adapters

The widget must look native inside a host design system. There are three layers. A host uses the first one
that is enough.

### 1. Token scope and theme presets

- The generated stylesheet declares the default `--hbv-*` values on `:where(.hbv-mic, .hbv-pill)`, not on
  `:root`. A custom property that references another one resolves on the element that declares it. On the
  widget element, `var(--popover)` picks up the nearest host scope: `.dark`, `.light`, `[data-brand]` or
  `[data-theme]`. Specificity zero lets any host class override any token.
- `theme` is `'default' | 'shadcn' | 'evinvest'`, and the default is `'default'`. It is set in the listener
  config and can be overridden per component. Both root elements render it as `data-hbv-theme="<theme>"`.
- The preset files are `shared/themes/shadcn.css` and `shared/themes/evinvest.css`. They are hand-written, and
  every selector is wrapped in `:where(...)`. The generator appends them after the base CSS in the one
  generated stylesheet, so a host imports a single file. A preset uses only the host's CSS variables. It needs
  no Tailwind and no dependency on the kit.

Preset mapping (`<token>` ← host variable; tokens not listed keep their default):

| `--hbv-color-*` | shadcn (v4: oklch, raw `var()`) | evinvest (`@evinvest/uikit` ≥ 0.36, hex) |
| --- | --- | --- |
| `pill-bg` | `var(--popover)` | `var(--popover)` |
| `pill-border` | `var(--border)` | `var(--border)` |
| `pill-shadow` | `0 4px 16px color-mix(in oklab, var(--foreground) 12%, transparent)` | `none` (the kit has no shadows) |
| `text` | `var(--popover-foreground)` | `var(--ink)` |
| `text-muted` | `var(--muted-foreground)` | `var(--ink-mid)` |
| `error` | `var(--destructive)` | `var(--accent-error)` |
| `strand-0/1/2` | `var(--chart-1/2/3)` | `var(--chart-1/2/3)` |
| `dot` | `var(--popover-foreground)` | `var(--ink)` |
| `chip-bg` / `chip-bg-hover` | `var(--secondary)` / `var(--accent)` | `var(--tint-neutral-fill)` / `var(--hover)` |
| `chip-text` | `var(--secondary-foreground)` | `var(--ink)` |
| `mic-bg` / `mic-bg-hover` | `var(--secondary)` / `var(--accent)` | `var(--secondary)` / `var(--hover)` |
| `mic-bg-active` / `mic-icon` active | `var(--primary)` / `var(--primary-foreground)` | `var(--primary)` / `var(--on-primary)` |
| `mic-icon` | `var(--secondary-foreground)` | `var(--ink)` |
| `mic-ring` / `focus-ring` | `var(--ring)` | `var(--ring)` |

Also: `--hbv-radius-chip` ← `calc(var(--radius) - 4px)` (shadcn) or `var(--chip-radius)` (evinvest), and
`--hbv-font-family` ← `inherit` (shadcn) or `var(--font-sans)` (evinvest). The active mic needs a token
`--hbv-color-mic-icon-active`; add it to `design-tokens.json` with the default `#ffffff`. shadcn v3 hosts use
HSL triplets (`hsl(var(--popover))`); v3 is out of scope, and the README says how to override.

### Cascade layers

The generated stylesheet starts with `@layer theme, base, components, utilities;`, which is Tailwind v4's
own order statement. It then puts every widget rule, presets included, in `@layer components`.

- In a Tailwind host, utilities beat the widget rules without `!`, and preflight (`base`) cannot reset them.
  This holds whatever the import order, because the first order statement fixes the order.
- In a host without Tailwind, any unlayered host CSS beats the widget rules.
- The token defaults stay inside `:where(...)` (zero specificity).

`.hbv-lang--bare` resets a chip button that sits inside a kit badge. It sets a transparent background, no
border, no padding, and inherits colour and font. The Dioxus evinvest adapter uses it, because the kit's
`Badge` has no `asChild` there. `.hbv-pill__status` never shrinks; `.hbv-pill__text` takes the remaining
width and ends with an ellipsis.

### 2. Slots and your own button

- Every component accepts a root class (`className` in React, `class` in Dioxus). It also accepts a per-part
  map: `classNames` in React, a `class_names` struct in Rust. The parts are `root`, `ring` and `icon` for the
  mic, and `root`, `wave`, `strand`, `status`, `text`, `dots` and `lang` for the pill. The contract class
  names always stay. The host classes are added after them.
- React `MicButton` takes `asChild`. It then renders its single child element instead of its own `<button>`,
  merges `className`, composes `onClick`, forwards `ref`, and puts in all contract `aria-*` and `data-*`
  attributes and the ring and icon children. Use it with a shadcn
  `<Button variant="outline" size="icon">`. The implementation has no Radix dependency.
- React `ListenerPill` takes `components={{ LangButton }}`. That is a component that receives button props
  plus `children`.
- Headless helpers for hosts that compose their own UI are exported from both libraries: `wavePath(...)`
  (`wave_path`), a translator `t(lang, key, vars)`, `micButtonProps(listener)` (`mic_button_props`), which
  returns the aria, data and click props, and the standalone `Waveform`.
- Rust has no `asChild`. `MicButton` and `ListenerPill` take the `class` and `class_names` overrides, and
  `mic_button_props` lets a host render any element.

### 3. Native adapters

Rule: an adapter never adds its own Tailwind utility classes. Tailwind cannot scan a published package. An
adapter uses only the kit's own class builders and components, plus the `hbv-*` CSS.

- **`happybirthdayvalera/evinvest`** (npm subpath export; `@evinvest/uikit` `>=0.36 <1` is an optional peer
  dependency). It exports `EvMicButton` and `EvListenerPill`, and sets `theme: 'evinvest'`. The mic is a
  native `<button>` with `className={cn(buttonVariants({ variant, size, icon: true }), 'hbv-mic')}`. Do not
  use their `Button`, so that all aria attributes stay. The lang chip uses their `Badge` with `asChild` over
  a `<button>`. The main entry never imports this subpath.
- **shadcn registry item** in `registry/` (`registry.json` + `registry/voice-listener/voice-listener.tsx`).
  It is copied into the host by `npx shadcn add <url>`. It imports `@/components/ui/button` and
  `@/lib/utils`, and it uses Tailwind classes on the shadcn tokens over the headless `useListener` and
  `Waveform`. It is built to `registry/public/r/*.json` with the shadcn CLI. The URL works once the repo is
  public.
- **Rust feature `ev_uikit`**: optional dependency
  `ev_lib = { version = "0.34", default-features = false, features = ["uikit_dioxus"] }` and the module
  `happybirthdayvalera::ev` with `EvMicButton` and `EvListenerPill`. `ev_lib::uikit_dioxus::Button` has no
  aria passthrough. If `ev_lib` exposes its button class builder (`ev_lib_classes`), render a native
  `button { class: <builder>, aria_pressed, aria_label, … }`. If it does not, use their `Button` and record
  the a11y gap as an upstream request to `EV-Main/web_std`. A host on this kit already runs Tailwind with
  `ev_lib_classes::CLASS_INVENTORY`. The adapter must therefore use only class strings from that inventory.

## Easter egg

Off by default (`easterEgg: false`). Normalize the final text: lowercase, replace `ё` with `е`, replace every
character that is not a letter, digit or space with a space, and collapse whitespace. If the result contains
any phrase from `birthdayPhrases` of any language, set `celebrating` for `easterEggMs`. While it is set:

- `pillVisible` is true.
- The pill and the mic button both get `data-celebrate="true"`.
- The pill shows the final text in `.hbv-pill__text`.
- The waveform keeps animating at `A = idleAmp + (1 − idleAmp) · 0.6`.
- CSS animates a rainbow on the strands.

A new `START` ends the celebration.

## Generated files

`just sync-assets` writes these. Never edit them by hand; CI fails when a committed copy is stale.

| Output | Source |
| --- | --- |
| `packages/react/src/styles.css` | `design-tokens.json` + `styles/listener.css` |
| `crates/happybirthdayvalera/assets/listener.css` | the same, byte for byte |
| `crates/happybirthdayvalera/assets/whisper-worker.js` | the worker entry in `packages/react/src/engines/whisper/` |
| `crates/happybirthdayvalera/shared/*.json` | byte-for-byte copies of `locales`, `design-tokens`, `behavior` and `machine`.json |
| `shared/wave-fixture.json` | `scripts/wave-fixture.mjs` over `design-tokens.json` |

The crate reads its copies with `include_str!("../shared/…")`, because `cargo publish` packages only the crate
directory. Each CSS and JS output starts with a `GENERATED by scripts/sync-assets — do not edit by hand`
comment. JSON cannot hold a comment, so the drift check alone guards the JSON copies.
