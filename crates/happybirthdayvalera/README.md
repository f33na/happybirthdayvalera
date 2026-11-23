# happybirthdayvalera

> **For Valera, on your birthday.** May everything you say be heard, and understood, in every language you
> speak.

Voice input for [Dioxus](https://dioxuslabs.com) 0.7: a mic button and a dark pill with a live three-strand
waveform, a localized status and a language chip. It does real speech-to-text in **English, Russian, German
and French** with Whisper, and it runs **locally**. No audio leaves the device.

```rust,ignore
use dioxus::prelude::*;
use happybirthdayvalera::{use_listener, ListenerConfig, ListenerPill, MicButton};

#[component]
fn Toolbar() -> Element {
    let listener = use_listener(ListenerConfig {
        on_result: Some(std::rc::Rc::new(|text, lang| println!("{lang}: {text}"))),
        ..Default::default()
    });
    rsx! {
        MicButton { listener }
        // Mount the pill on pill_visible(): it stays up for "Didn't catch that" and the easter egg.
        if listener.pill_visible() {
            ListenerPill { listener }
        }
    }
}
```

## Features

The crate does not enable a Dioxus renderer; your app does. Pick the feature that matches it:

| Feature | Capture | Engine |
| --- | --- | --- |
| `web` | `getUserMedia` and an `AudioWorklet` that resamples to 16 kHz | Whisper (transformers.js) in a module Web Worker |
| `desktop` | `cpal`, resampled to 16 kHz on a capture thread | whisper.cpp through `whisper-rs` on a worker thread |
| `metal` | | `desktop` with whisper.cpp on the GPU (Apple) |
| `ev_uikit` | | `EvMicButton` and `EvListenerPill` over the evinvest uikit (needs nightly, see below) |

`ev_uikit` needs a nightly toolchain, or `RUSTC_BOOTSTRAP=1` on stable. Its dependencies `ev_lib` 0.34 and
`ev_lib_classes` 0.20 turn on the nightly-only `#![feature(default_field_values)]`, so a stable build fails
with E0554. This stays so until EV-Main/web_std drops that feature from both crates. The other features
build on stable.

The docs on docs.rs are built with `web` and `ev_uikit`. The docs.rs image has no ALSA or webkit
libraries, so the `desktop` items (`WhisperEngine`, `ModelStore`) are not there; run
`cargo doc --features desktop` to see them.

A typical app maps its own features to both:

```toml
[features]
web = ["dioxus/web", "happybirthdayvalera/web"]
desktop = ["dioxus/desktop", "happybirthdayvalera/desktop"]
```

The `desktop` feature builds whisper.cpp, so it needs `cmake` and a C++ compiler. On Linux, `cpal` also needs
the ALSA headers (`libasound2-dev`).

## Models

The default model is Whisper `tiny`; set `ListenerConfig::model` to `ModelSize::Base` or `ModelSize::Small` for
better accuracy at a larger download.

- **Desktop** downloads `ggml-<size>.bin` from `huggingface.co/ggerganov/whisper.cpp` on the first `START` into
  `<cache dir>/happybirthdayvalera/models`. The pill shows the progress. The file is written to a `.part` file,
  checked against a SHA-256 pinned in the crate and renamed only after the check.
- **Web** loads `onnx-community/whisper-<size>` in the Web Worker; the browser caches it. The q8 `tiny`
  files are about 41 MB, plus about 20 MB of onnxruntime wasm from jsDelivr.

### Self-hosted models

Set `ListenerConfig::model_base_url` to load every file from your own host instead of the Hugging Face Hub
and jsDelivr. The default engine adds a missing trailing `/` and expects this layout under the base URL:

- `<base>onnx-community/whisper-<size>/…`: the transformers.js model files (web);
- `<base>ort/`: the onnxruntime-web `.wasm` and `.mjs` files (web), unless the engine sets `wasm_paths`;
- `<base>ggml-<size>.bin`: the whisper.cpp files (desktop). The download is still checked against the pinned
  SHA-256, and the file goes to the same cache directory.

The option counts on the first render only, and it is ignored when `ListenerConfig::engine` is set. For a
custom engine, use `WorkerEngine::model_base_url` (web) or `ModelStore::with_base_url` (desktop).

macOS asks for microphone access on the first capture. A bundled desktop app needs
`NSMicrophoneUsageDescription` in its `Info.plist`.

## Behaviour

- **Language.** A forced `ListenerConfig::lang`, else the stored choice (web, with `persist_lang`), else the
  first supported language of the browser or OS locales, else English. The chip cycles EN → RU → DE → FR.
  Whisper always gets the language as a forced token.
- **Dictation.** The widget records while it listens and transcribes when you stop. After speech, about 1.5 s
  of silence stops it by itself; 30 s is the limit. A recording without speech, or under 0.3 s, skips the
  engine and shows "Didn't catch that".
- **Progress.** The pill shows "Loading model 42%" while the engine reports a fraction, and "Loading
  model…" while the size is unknown; it never invents a percentage.
- **Motion.** The waveform and the mic level animate inside the page, so no Rust render or desktop IPC message
  happens per frame. `prefers-reduced-motion: reduce` draws a still waveform once per state change.
- **Easter egg.** With `easter_egg: true`, a birthday wish in any of the four languages makes the strands
  celebrate for 3 s: the pill stays visible with the text, and both the pill and the mic get
  `data-celebrate="true"`.
- **Microphone access (desktop).** On macOS the widget asks for access before it opens the microphone, and
  stays in "requesting" while the system dialog is open. A refusal shows the "microphone denied" error. On
  every desktop system, an input that delivers only exact zeros for the first 0.7 s is a failed microphone
  (denied on macOS, unavailable elsewhere), not "Didn't catch that". A quiet room through a working
  microphone is never bit-exact zero, so it still ends in "Didn't catch that".

## Troubleshooting

- **macOS says the microphone is denied, but you never saw a dialog.** macOS asks once per app and remembers
  the answer. When `dx serve --platform desktop` launches the app, macOS often asks for the app that started
  it, such as your terminal or editor. Open System Settings → Privacy & Security → Microphone and turn it on
  for that app, then start the widget again.
- **A bundled macOS app is denied without a dialog.** The bundle needs `NSMicrophoneUsageDescription` in
  its Info.plist. With `dx` 0.7, add it to `Dioxus.toml`:

  ```toml
  [permissions]
  microphone = { description = "Turns your speech into text on this device." }
  ```

## Theming

- `ListenerConfig::theme` (`Theme::Default`, `Theme::Shadcn`, `Theme::Evinvest`) renders as
  `data-hbv-theme`; the stylesheet maps the presets to the host's CSS variables. Each component can override
  it with its `theme` prop.
- `class` and `class_names` (`MicClassNames`, `PillClassNames`) add host classes after the contract classes.
- Headless helpers for your own markup: `mic_button_props`, `pill_props`, `wave_path`, `t`, `MicParts` and
  the standalone `Waveform`.

## Your own engine

Implement `SpeechEngine` and pass it as `ListenerConfig::engine: Some(EngineHandle::new(MyEngine))`:

```rust,ignore
use happybirthdayvalera::{EngineError, Lang, ModelSize, SpeechEngine};

struct Echo;

impl SpeechEngine for Echo {
    async fn load(&self, _model: ModelSize, on_progress: &dyn Fn(f32)) -> Result<(), EngineError> {
        on_progress(1.0);
        Ok(())
    }

    async fn transcribe(&self, pcm: Vec<f32>, _lang: Lang) -> Result<String, EngineError> {
        Ok(format!("{} samples", pcm.len()))
    }
}
```

## License

MIT
