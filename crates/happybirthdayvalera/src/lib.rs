#![doc = include_str!("../README.md")]

mod components;
mod controller;
pub mod easter;
pub mod engine;
pub mod i18n;
pub mod lang;
mod listener;
pub mod machine;
mod platform;
pub mod resample;
mod shared;
pub mod theme;
pub mod vad;
pub mod wave;

pub use components::{
    ListenerPill, MicButton, MicAttrs, MicParts, PillAttrs, STYLESHEET, Waveform, mic_button_props,
    pill_props, wave_path,
};
pub use engine::{EngineError, EngineHandle, ModelSize, SpeechEngine};
pub use i18n::{Msg, format as t};
pub use lang::{Lang, resolve_preferred_lang};
pub use listener::{Listener, ListenerConfig, TextHandler, status_text, use_listener};
pub use machine::{ErrorCode, Event, State};
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub use platform::{ModelStore, WhisperEngine};
pub use theme::{MicClassNames, PillClassNames, Theme};
