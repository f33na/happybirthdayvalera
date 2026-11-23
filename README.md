# happybirthdayvalera

> **For Valera, on your birthday.** May everything you say be heard, and understood, in every language you
> speak. 🎂

A small voice-input widget: a mic button and a dark pill with a live waveform, a "Listening" status and a
language chip. It does real speech-to-text in **English, Russian, German and French**, and by default it runs
**locally** with Whisper. No audio leaves the device.

It ships in two flavours with one name and one look:

| Flavour | Package | Path |
| --- | --- | --- |
| TypeScript + React 18/19 | [`happybirthdayvalera` on npm](https://www.npmjs.com/package/happybirthdayvalera) | [`packages/react`](packages/react) |
| Rust + Dioxus 0.7 (web and desktop) | [`happybirthdayvalera` on crates.io](https://crates.io/crates/happybirthdayvalera) | [`crates/happybirthdayvalera`](crates/happybirthdayvalera) |

## Quick look

React:

```tsx
import { MicButton, ListenerPill, useListener } from 'happybirthdayvalera';
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

Dioxus:

```rust
use dioxus::prelude::*;
use happybirthdayvalera::{use_listener, ListenerConfig, ListenerPill, MicButton};

#[component]
fn Toolbar() -> Element {
    let listener = use_listener(ListenerConfig::default());
    rsx! {
        MicButton { listener }
        ListenerPill { listener }
    }
}
```

## How it works

- **Language.** The default comes from the browser or OS locale and falls back to English. The user can switch
  it with the chip on the pill. The app can force it with a prop.
- **Recognition.** Whisper (`base` by default) gets the language as a forced token. On the web it runs with
  transformers.js in a Web Worker. On desktop it runs with whisper.cpp through `whisper-rs`. The model
  downloads once and stays in the cache.
- **Dictation.** The widget records while it listens and transcribes when you stop. It also stops by itself
  after about 1.5 s of silence.
- **Pluggable.** Bring your own engine through the `SpeechEngine` interface. The React package also has a light
  Web Speech API engine that needs no model download.

The shared contract for both libraries is in [`shared/CONTRACT.md`](shared/CONTRACT.md).

## Development

You need node 22, a Rust toolchain, `dx` 0.7, `cmake` and `just`.

```sh
just            # list recipes
just test       # TypeScript and Rust unit tests
just demo-react # Vite demo
just demo-web   # Dioxus web demo
just demo-desktop
just sync-assets && just check-drift
```

## Design note

The pill is an original design. It is inspired by voice inputs in general, and it uses no code, assets or
copy from any other product.

## License

MIT
