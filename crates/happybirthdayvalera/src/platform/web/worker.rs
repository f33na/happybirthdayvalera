//! The default web engine: Whisper (transformers.js) in the module Web Worker of the shared asset, spoken to
//! through the worker protocol of the contract.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

//  with options expands to paths under , which the prelude re-exports.
// `asset!` with options expands to paths under `manganis`, which the prelude re-exports.
use dioxus::prelude::{Asset, AssetOptions, asset, manganis};
use futures_channel::mpsc::{UnboundedSender, unbounded};
use futures_channel::oneshot;
use futures_util::{FutureExt as _, StreamExt as _, select};
use js_sys::{Array, Float32Array, Object, Reflect};
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::{ErrorEvent, MessageEvent, Worker, WorkerOptions, WorkerType};

use crate::engine::{EngineError, ModelSize, SpeechEngine};
use crate::lang::Lang;

/// The worker is a finished ES module from the TS build, so `dx` must ship it byte for byte under a hashed name.
/// `dx` 0.7.9 minifies plain and default JS assets and re-bundles `with_module(true)` ones; only
/// `with_minify(false)` without `with_module` leaves the file untouched (checked on a `dx build` output).
const WORKER_JS: Asset = asset!("/assets/whisper-worker.js", AssetOptions::js().with_minify(false));

type LoadResult = Result<(), EngineError>;
type TextResult = Result<String, EngineError>;

#[derive(Default)]
struct Inner {
    worker: Option<Worker>,
    on_message: Option<Closure<dyn FnMut(MessageEvent)>>,
    on_error: Option<Closure<dyn FnMut(ErrorEvent)>>,
    loaded: Option<&'static str>,
    loading: Option<&'static str>,
    load_waiters: Vec<oneshot::Sender<LoadResult>>,
    progress: Vec<UnboundedSender<f32>>,
    next_id: u32,
    pending: HashMap<u32, oneshot::Sender<TextResult>>,
}

impl Inner {
    /// Fails every waiter and forgets the worker, so that the next call starts a fresh one.
    fn fail_all(&mut self, message: &str) {
        if let Some(worker) = self.worker.take() {
            worker.terminate();
        }
        // The callbacks stay until the next worker replaces them: this may run inside `on_error` itself.
        self.loaded = None;
        self.loading = None;
        self.progress.clear();
        for waiter in self.load_waiters.drain(..) {
            // A dropped receiver means that caller no longer waits.
            let _ = waiter.send(Err(EngineError::new(message)));
        }
        for (_, waiter) in self.pending.drain() {
            let _ = waiter.send(Err(EngineError::new(message)));
        }
    }
}

/// Whisper in a module Web Worker. The default worker is the crate's `whisper-worker.js` asset.
pub struct WorkerEngine {
    url: String,
    wasm_paths: Option<String>,
    inner: Rc<RefCell<Inner>>,
}

impl Default for WorkerEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerEngine {
    /// An engine over the bundled worker asset. The worker starts on the first call.
    #[must_use]
    pub fn new() -> Self {
        Self::with_worker_url(WORKER_JS.to_string())
    }

    /// An engine over a worker script that speaks the same protocol.
    pub fn with_worker_url(url: impl Into<String>) -> Self {
        Self { url: url.into(), wasm_paths: None, inner: Rc::default() }
    }

    /// Sends `wasmPaths` (the base URL of the onnxruntime wasm files) with every `load`, for offline hosts.
    #[must_use]
    pub fn wasm_paths(mut self, base_url: impl Into<String>) -> Self {
        self.wasm_paths = Some(base_url.into());
        self
    }

    fn worker(&self) -> Result<Worker, EngineError> {
        let mut inner = self.inner.borrow_mut();
        if let Some(worker) = &inner.worker {
            return Ok(worker.clone());
        }
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        let worker = Worker::new_with_options(&self.url, &options)
            .map_err(|e| EngineError::new(format!("cannot start the worker {}: {e:?}", self.url)))?;

        let weak = Rc::downgrade(&self.inner);
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            on_message(&weak, &event.data());
        });
        let weak = Rc::downgrade(&self.inner);
        let url = self.url.clone();
        let on_error = Closure::<dyn FnMut(ErrorEvent)>::new(move |event: ErrorEvent| {
            let message = format!("the worker {url} failed: {}", event.message());
            tracing::warn!("{message}");
            if let Some(inner) = weak.upgrade() {
                inner.borrow_mut().fail_all(&message);
            }
        });
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        inner.worker = Some(worker.clone());
        inner.on_message = Some(on_message);
        inner.on_error = Some(on_error);
        Ok(worker)
    }
}

fn field(data: &JsValue, key: &str) -> JsValue {
    Reflect::get(data, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED)
}

fn on_message(weak: &Weak<RefCell<Inner>>, data: &JsValue) {
    let Some(inner) = weak.upgrade() else { return };
    let mut inner = inner.borrow_mut();
    let kind = field(data, "type").as_string().unwrap_or_default();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "ids are u32 values that we sent")]
    let id = field(data, "id").as_f64().map(|id| id as u32);
    match kind.as_str() {
        "progress" => {
            // The worker leaves `fraction` out while any file size is unknown; the progress stays unknown then.
            if let Some(fraction) = field(data, "fraction").as_f64() {
                #[allow(clippy::cast_possible_truncation, reason = "a progress fraction")]
                let fraction = fraction.clamp(0.0, 1.0) as f32;
                inner.progress.retain(|tx| tx.unbounded_send(fraction).is_ok());
            }
        }
        "ready" => {
            inner.loaded = inner.loading.take().or(inner.loaded);
            inner.progress.clear();
            for waiter in inner.load_waiters.drain(..) {
                // A dropped receiver means that caller no longer waits.
                let _ = waiter.send(Ok(()));
            }
        }
        "result" => {
            let text = field(data, "text").as_string().unwrap_or_default();
            if let Some(waiter) = id.and_then(|id| inner.pending.remove(&id)) {
                let _ = waiter.send(Ok(text.trim().to_owned()));
            }
        }
        "error" => {
            let message = field(data, "message").as_string().unwrap_or_else(|| "unknown worker error".into());
            if let Some(waiter) = id.and_then(|id| inner.pending.remove(&id)) {
                let _ = waiter.send(Err(EngineError::new(message)));
            } else {
                // An error without an id is a load error.
                inner.loading = None;
                inner.progress.clear();
                for waiter in inner.load_waiters.drain(..) {
                    let _ = waiter.send(Err(EngineError::new(message.clone())));
                }
            }
        }
        other => tracing::debug!("ignoring worker message type `{other}`"),
    }
}

impl SpeechEngine for WorkerEngine {
    async fn load(&self, model: ModelSize, on_progress: &dyn Fn(f32)) -> Result<(), EngineError> {
        let model_id = model.web_model_id();
        let worker = self.worker()?;
        let (done_tx, done_rx) = oneshot::channel();
        let (progress_tx, progress_rx) = unbounded();
        {
            let mut inner = self.inner.borrow_mut();
            if inner.loaded == Some(model_id) && inner.loading.is_none() {
                on_progress(1.0);
                return Ok(());
            }
            inner.load_waiters.push(done_tx);
            inner.progress.push(progress_tx);
            if inner.loading != Some(model_id) {
                inner.loading = Some(model_id);
                let msg = Object::new();
                set(&msg, "type", &JsValue::from_str("load"));
                set(&msg, "model", &JsValue::from_str(model_id));
                if let Some(paths) = &self.wasm_paths {
                    set(&msg, "wasmPaths", &JsValue::from_str(paths));
                }
                if let Err(e) = worker.post_message(&msg) {
                    inner.fail_all(&format!("cannot post `load`: {e:?}"));
                }
            }
        }
        let mut progress = progress_rx.fuse();
        let mut done = done_rx.fuse();
        loop {
            select! {
                fraction = progress.next() => if let Some(fraction) = fraction { on_progress(fraction) },
                result = done => {
                    return result.unwrap_or_else(|oneshot::Canceled| Err(EngineError::new("the worker stopped")));
                }
            }
        }
    }

    async fn transcribe(&self, pcm: Vec<f32>, lang: Lang) -> Result<String, EngineError> {
        let worker = self.worker()?;
        let (done_tx, done_rx) = oneshot::channel();
        {
            let mut inner = self.inner.borrow_mut();
            let id = inner.next_id;
            inner.next_id = inner.next_id.wrapping_add(1);
            inner.pending.insert(id, done_tx);

            let samples = Float32Array::new_with_length(u32::try_from(pcm.len()).unwrap_or(u32::MAX));
            samples.copy_from(&pcm[..samples.length() as usize]);
            let msg = Object::new();
            set(&msg, "type", &JsValue::from_str("transcribe"));
            set(&msg, "id", &JsValue::from(id));
            set(&msg, "pcm", &samples);
            set(&msg, "lang", &JsValue::from_str(lang.code()));
            // The buffer moves to the worker instead of being copied.
            if let Err(e) = worker.post_message_with_transfer(&msg, &Array::of1(&samples.buffer()))
                && let Some(waiter) = inner.pending.remove(&id)
            {
                let _ = waiter.send(Err(EngineError::new(format!("cannot post `transcribe`: {e:?}"))));
            }
        }
        done_rx.await.unwrap_or_else(|oneshot::Canceled| Err(EngineError::new("the worker stopped")))
    }

    fn dispose(&self) {
        self.inner.borrow_mut().fail_all("the engine was disposed");
    }
}

fn set(target: &Object, key: &str, value: &JsValue) {
    // `Reflect.set` on a plain new object cannot fail.
    let _ = Reflect::set(target, &JsValue::from_str(key), value);
}
