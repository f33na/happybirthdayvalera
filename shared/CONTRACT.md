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
- At `maxDurationMs` the controller sends `STOP`.
- When `interimEveryMs > 0`, the controller transcribes the whole buffer so far at that interval while
  listening. It skips a tick if the previous interim call has not finished. Interim text is shown, but
  `onResult` fires only for the final text.

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
{ type: 'load', model: 'onnx-community/whisper-base', wasmPaths?: string } // wasmPaths: onnxruntime wasm base URL; default is the CDN
{ type: 'transcribe', id: number, pcm: Float32Array, lang: Lang }   // pcm buffer is transferred
```

Worker → main:

```ts
{ type: 'progress', fraction?: number }         // 0..1 over all model files; absent while any size is unknown
{ type: 'ready' }
{ type: 'result', id: number, text: string }
{ type: 'error', id?: number, message: string } // id is absent for a load error
```

A second `load` with the same model answers `ready` at once.

## Public API

TypeScript:

```tsx
const listener = useListener({ lang?, persistLang?, engine?, model?, onResult?, onInterim?,
                               interimEveryMs?, silenceMs?, easterEgg?, preload?, theme? });
// listener: { state, error, progress, text, interim, lang, setLang, cycleLang,
//             start, stop, toggle, reset, celebrating, levelRef, theme,
//             nothingHeard, langLocked, pillVisible }  (progress: number | null)
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
