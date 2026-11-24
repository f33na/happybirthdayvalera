#![doc = include_str!("../README.md")]

mod components;
mod controller;
mod dictation;
pub mod easter;
pub mod engine;
#[cfg(feature = "ev_uikit")]
pub mod ev;
mod gesture;
pub mod i18n;
pub mod lang;
mod listener;
pub mod machine;
mod platform;
pub mod resample;
mod session;
mod shared;
pub mod theme;
pub mod vad;
pub mod wave;

pub use components::{
    ListenerPill, MicButton, MicAttrs, MicParts, PillAttrs, STYLESHEET, Waveform, mic_button_props,
    pill_props, wave_path,
};
pub use dictation::use_dictation;
pub use engine::{Device, EngineError, EngineHandle, ModelSize, SpeechEngine};
pub use i18n::{Msg, format as t};
pub use lang::{Lang, resolve_preferred_lang};
pub use listener::{Listener, ListenerConfig, Mode, TextHandler, UnknownMode, status_text, use_listener};
pub use machine::{ErrorCode, Event, State};
#[cfg(all(feature = "web", any(target_arch = "wasm32", not(feature = "desktop"))))]
pub use platform::WorkerEngine;
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub use platform::{ModelStore, WhisperEngine};
pub use theme::{MicClassNames, PillClassNames, Theme};
