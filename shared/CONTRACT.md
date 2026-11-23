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
  dispose?(): void;
}
```

Rust has the same trait with async methods. Whisper always gets the language as a forced token, with task
`transcribe`, not `translate`. The engine trims the result.

Model ids are in `behavior.json` → `models`. Desktop checks the SHA-256 of a downloaded `ggml-*.bin` against
pinned values in code. It writes to a `.part` file and renames it after the check.

### Web worker protocol

The default web engine runs Whisper (transformers.js) in a module Web Worker. The same worker file serves the
npm package and the Dioxus web build (`crates/happybirthdayvalera/assets/whisper-worker.js`). Messages:

Main → worker:

```ts
{ type: 'load', model: 'onnx-community/whisper-base' }
{ type: 'transcribe', id: number, pcm: Float32Array, lang: Lang }   // pcm buffer is transferred
```

Worker → main:

```ts
{ type: 'progress', fraction: number }          // 0..1, aggregated over all model files
{ type: 'ready' }
{ type: 'result', id: number, text: string }
{ type: 'error', id?: number, message: string } // id is absent for a load error
```

A second `load` with the same model answers `ready` at once.

## Public API

TypeScript:

```tsx
const listener = useListener({ lang?, persistLang?, engine?, model?, onResult?, onInterim?,
                               interimEveryMs?, silenceMs?, easterEgg?, preload? });
// listener: { state, error, progress, text, interim, lang, setLang, cycleLang,
//             start, stop, toggle, reset, celebrating, levelRef }
<MicButton listener={listener} />
<ListenerPill listener={listener} />
```

Rust (Dioxus 0.7):

```rust
let listener = use_listener(ListenerConfig { lang: None, model: ModelSize::Base, ..Default::default() });
rsx! { MicButton { listener } ListenerPill { listener } }
```

`Listener` is a `Copy` handle with signals. Cargo features: `web` and `desktop`. The crate does not enable a
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

## Easter egg

Off by default (`easterEgg: false`). Normalize the final text: lowercase, replace `ё` with `е`, replace every
character that is not a letter, digit or space with a space, and collapse whitespace. If the result contains
any phrase from `birthdayPhrases` of any language, set `celebrating` for `easterEggMs`. The pill gets
`data-celebrate="true"`, and CSS animates a rainbow on the strands.

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
