//! The browser adapter: `getUserMedia` with an `AudioWorklet`, and Whisper in a module Web Worker.

mod capture;
mod worker;

pub(crate) use capture::{Capture, open_capture};
pub use worker::WorkerEngine;

use std::cell::RefCell;
use std::collections::HashMap;

use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::Closure;

use crate::engine::EngineHandle;
use crate::lang::Lang;

thread_local! {
    // One worker per page, so every listener shares one loaded model instead of one each.
    static ENGINE: EngineHandle = EngineHandle::new(WorkerEngine::new());
    // The same rule per self-hosted base URL: listeners that name one host share its worker.
    static HOSTED: RefCell<HashMap<String, EngineHandle>> = RefCell::new(HashMap::new());
}

/// `model_base_url` arrives normalized, so one host maps to one key.
pub(crate) fn default_engine(model_base_url: Option<String>) -> EngineHandle {
    let Some(base) = model_base_url else { return ENGINE.with(Clone::clone) };
    HOSTED.with_borrow_mut(|hosted| {
        hosted
            .entry(base)
            .or_insert_with_key(|base| EngineHandle::new(WorkerEngine::new().model_base_url(base)))
            .clone()
    })
}

/// `navigator.languages`, then `navigator.language`.
pub(crate) fn preferred_locales() -> Vec<String> {
    let Some(window) = web_sys::window() else { return Vec::new() };
    let navigator = window.navigator();
    let mut out: Vec<String> = navigator.languages().iter().filter_map(|v| v.as_string()).collect();
    if let Some(lang) = navigator.language() {
        out.push(lang);
    }
    out
}

fn storage() -> Option<web_sys::Storage> {
    // `localStorage` throws in some privacy modes; the language then simply is not persisted.
    web_sys::window()?.local_storage().ok().flatten()
}

pub(crate) fn load_lang(key: &str) -> Option<Lang> {
    let value = storage()?.get_item(key).ok().flatten()?;
    Lang::from_code(&value)
}

pub(crate) fn store_lang(key: &str, lang: Lang) {
    if let Some(storage) = storage()
        && let Err(e) = storage.set_item(key, lang.code())
    {
        tracing::warn!("cannot persist the language under `{key}`: {e:?}");
    }
}

/// A `setTimeout` wrapped in a future. Dropping the future clears the timer, so the browser never calls a
/// freed closure.
pub(crate) async fn sleep_ms(ms: u32) {
    struct Timer {
        window: web_sys::Window,
        id: i32,
        _callback: Closure<dyn FnMut()>,
    }
    impl Drop for Timer {
        fn drop(&mut self) {
            self.window.clear_timeout_with_handle(self.id);
        }
    }

    let (tx, rx) = futures_channel::oneshot::channel::<()>();
    let Some(window) = web_sys::window() else { return };
    let callback: Closure<dyn FnMut()> = Closure::once(move || {
        // The waiter may be gone (the listener unmounted); then nobody needs the wake-up.
        let _ = tx.send(());
    });
    let timeout = i32::try_from(ms).unwrap_or(i32::MAX);
    let id = match window
        .set_timeout_with_callback_and_timeout_and_arguments_0(callback.as_ref().unchecked_ref(), timeout)
    {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!("setTimeout failed: {e:?}");
            return;
        }
    };
    let _timer = Timer { window, id, _callback: callback };
    // A dropped sender also ends the wait, which is all a timer needs.
    let _ = rx.await;
}
