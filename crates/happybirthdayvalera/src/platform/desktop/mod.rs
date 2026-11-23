//! The native adapter: cpal capture and whisper.cpp through whisper-rs.

mod capture;
mod model;
mod permission;
mod whisper;

pub(crate) use capture::{Capture, open_capture};
pub use model::ModelStore;
pub use whisper::WhisperEngine;

use std::cell::RefCell;
use std::collections::HashMap;

use crate::engine::EngineHandle;
use crate::lang::Lang;

thread_local! {
    // One engine per UI thread, so every listener shares one loaded model instead of one each.
    static ENGINE: EngineHandle = EngineHandle::new(WhisperEngine::new());
    // The same rule per self-hosted base URL: listeners that name one host share its engine.
    static HOSTED: RefCell<HashMap<String, EngineHandle>> = RefCell::new(HashMap::new());
}

/// `model_base_url` arrives normalized, so one host maps to one key.
pub(crate) fn default_engine(model_base_url: Option<String>) -> EngineHandle {
    let Some(base) = model_base_url else { return ENGINE.with(Clone::clone) };
    HOSTED.with_borrow_mut(|hosted| {
        hosted
            .entry(base)
            .or_insert_with_key(|base| {
                EngineHandle::new(WhisperEngine::with_store(ModelStore::default().with_base_url(base)))
            })
            .clone()
    })
}

pub(crate) fn preferred_locales() -> Vec<String> {
    sys_locale::get_locales().collect()
}

/// Desktop has no language storage: the contract persists the language on the web only.
pub(crate) fn load_lang(_key: &str) -> Option<Lang> {
    None
}

pub(crate) fn store_lang(_key: &str, _lang: Lang) {}
