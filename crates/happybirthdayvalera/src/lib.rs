#![doc = include_str!("../README.md")]

pub mod easter;
pub mod engine;
pub mod i18n;
pub mod lang;
pub mod machine;
pub mod resample;
mod shared;
pub mod theme;
pub mod vad;
pub mod wave;

pub use engine::{EngineError, EngineHandle, ModelSize, SpeechEngine};
pub use i18n::{Msg, format as t};
pub use lang::{Lang, resolve_preferred_lang};
pub use machine::{ErrorCode, Event, State};
pub use theme::{MicClassNames, PillClassNames, Theme};
