//! The adapter for builds without a usable platform feature: the listener renders, and `START` fails with
//! `unsupported`.

use futures_channel::mpsc::UnboundedSender;

use crate::engine::{EngineError, EngineHandle, ModelSize, SpeechEngine};
use crate::lang::Lang;
use crate::machine::ErrorCode;

pub(crate) struct Capture;

#[allow(clippy::unused_async, reason = "the signature matches the async adapters of the other platforms")]
pub(crate) async fn open_capture(_sink: UnboundedSender<Vec<f32>>) -> Result<Capture, ErrorCode> {
    Err(ErrorCode::Unsupported)
}

pub(crate) fn default_engine(_model_base_url: Option<String>, _device: crate::engine::Device) -> EngineHandle {
    EngineHandle::new(UnsupportedEngine)
}

pub(crate) fn preferred_locales() -> Vec<String> {
    Vec::new()
}

pub(crate) fn load_lang(_key: &str) -> Option<Lang> {
    None
}

pub(crate) fn store_lang(_key: &str, _lang: Lang) {}

/// An engine for builds without a platform feature: every call fails.
struct UnsupportedEngine;

impl SpeechEngine for UnsupportedEngine {
    async fn load(&self, _model: ModelSize, _on_progress: &dyn Fn(f32)) -> Result<(), EngineError> {
        Err(EngineError::with_code(
            ErrorCode::Unsupported,
            "happybirthdayvalera was built without the `web` or `desktop` feature",
        ))
    }

    async fn transcribe(&self, _pcm: Vec<f32>, _lang: Lang) -> Result<String, EngineError> {
        Err(EngineError::new("happybirthdayvalera was built without the `web` or `desktop` feature"))
    }
}
