//! The native adapter: cpal capture and whisper.cpp through whisper-rs.

mod capture;
mod model;
mod whisper;

pub(crate) use capture::{Capture, open_capture};
pub use model::ModelStore;
pub use whisper::WhisperEngine;

use crate::engine::EngineHandle;
use crate::lang::Lang;

thread_local! {
    // One engine per UI thread, so every listener shares one loaded model instead of one each.
    static ENGINE: EngineHandle = EngineHandle::new(WhisperEngine::new());
}

pub(crate) fn default_engine() -> EngineHandle {
    ENGINE.with(Clone::clone)
}

pub(crate) fn preferred_locales() -> Vec<String> {
    sys_locale::get_locales().collect()
}

/// Desktop has no language storage: the contract persists the language on the web only.
pub(crate) fn load_lang(_key: &str) -> Option<Lang> {
    None
}

pub(crate) fn store_lang(_key: &str, _lang: Lang) {}
