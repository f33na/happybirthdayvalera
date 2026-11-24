//! The pluggable speech-to-text engine.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::str::FromStr;

use crate::lang::Lang;
use crate::machine::ErrorCode;
use crate::shared::BEHAVIOR;

/// A Whisper model size. The default comes from `behavior.json` → `models.default`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModelSize {
    /// About 41 MB on the web (q8) and 78 MB on desktop; fastest, least accurate; the default.
    Tiny,
    /// About 77 MB on the web (q8) and 148 MB on desktop.
    Base,
    /// About 466 MB.
    Small,
}

impl Default for ModelSize {
    fn default() -> Self {
        BEHAVIOR.models.default.parse().unwrap_or(ModelSize::Tiny)
    }
}

impl ModelSize {
    /// Every size.
    pub const ALL: [ModelSize; 3] = [ModelSize::Tiny, ModelSize::Base, ModelSize::Small];

    /// `tiny`, `base` or `small`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ModelSize::Tiny => "tiny",
            ModelSize::Base => "base",
            ModelSize::Small => "small",
        }
    }

    /// The transformers.js model id for the web worker, for example `onnx-community/whisper-base`.
    #[must_use]
    pub fn web_model_id(self) -> &'static str {
        let web = &BEHAVIOR.models.web;
        match self {
            ModelSize::Tiny => &web.tiny,
            ModelSize::Base => &web.base,
            ModelSize::Small => &web.small,
        }
    }

    /// The whisper.cpp file name, for example `ggml-base.bin`.
    #[must_use]
    pub fn ggml_file_name(self) -> &'static str {
        let desktop = &BEHAVIOR.models.desktop;
        match self {
            ModelSize::Tiny => &desktop.tiny,
            ModelSize::Base => &desktop.base,
            ModelSize::Small => &desktop.small,
        }
    }

    /// The download URL of the whisper.cpp file on the Hugging Face Hub.
    #[must_use]
    pub fn ggml_url(self) -> String {
        format!("{}{}", BEHAVIOR.models.desktop.base_url, self.ggml_file_name())
    }
}

/// Where the web worker runs Whisper. Desktop ignores it: whisper.cpp picks its backend at build time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Device {
    /// The worker decides: WebGPU for `base` when the models are self-hosted and the browser has an adapter,
    /// else WebAssembly. The `load` message then carries no `device`.
    #[default]
    Auto,
    /// WebGPU; a failure to load does not fall back.
    Webgpu,
    /// WebAssembly, threaded when the page is cross-origin isolated.
    Wasm,
}

impl Device {
    /// `auto`, `webgpu` or `wasm`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Device::Auto => "auto",
            Device::Webgpu => "webgpu",
            Device::Wasm => "wasm",
        }
    }
}

/// Adds the trailing `/` that the model URL layout needs: every file URL is the base plus a relative path.
pub(crate) fn normalize_base_url(url: &str) -> String {
    if url.ends_with('/') { url.to_owned() } else { format!("{url}/") }
}

impl fmt::Display for ModelSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ModelSize {
    type Err = EngineError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ModelSize::ALL
            .into_iter()
            .find(|size| size.as_str() == s)
            .ok_or_else(|| EngineError::new(format!("unknown model size `{s}`")))
    }
}

/// An engine failure. The listener maps a load failure to its [`code`](EngineError::code), else to
/// `model-failed`, and a transcription failure to `transcribe-failed`; the message goes to the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineError {
    message: String,
    code: Option<ErrorCode>,
}

impl EngineError {
    /// An error with a human-readable cause.
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), code: None }
    }

    /// An error that the UI shows as `code`, for example [`ErrorCode::Unsupported`] from a load.
    pub fn with_code(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { message: message.into(), code: Some(code) }
    }

    /// The cause.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The error code that the engine chose, if any.
    #[must_use]
    pub fn code(&self) -> Option<ErrorCode> {
        self.code
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for EngineError {}

/// A speech-to-text engine.
///
/// The listener calls `load` before each recording; a loaded engine returns at once. It calls `begin` on
/// entering `listening`. `transcribe` gets mono 16 kHz samples and must force `lang` (no language detection,
/// no translation) and trim the text. An engine that captures audio itself starts in `begin` and ignores
/// `pcm`. The listener does not call `transcribe` when the VAD heard no speech or the audio is under 0.3 s.
///
/// The futures are not `Send`: the listener drives them on the UI thread, and the web engines hold
/// JavaScript values. Wrap an engine in [`EngineHandle::new`] to pass it to the listener.
#[allow(async_fn_in_trait, reason = "the futures are polled on the UI thread only, so no caller needs a Send bound")]
pub trait SpeechEngine: 'static {
    /// Downloads and loads `model`, reporting progress as a fraction in 0..=1.
    async fn load(&self, model: ModelSize, on_progress: &dyn Fn(f32)) -> Result<(), EngineError>;

    /// Transcribes `pcm` (mono f32 at 16 kHz) in `lang`.
    async fn transcribe(&self, pcm: Vec<f32>, lang: Lang) -> Result<String, EngineError>;

    /// Called on entering `listening`, before any audio. The default does nothing.
    fn begin(&self, _lang: Lang) {}

    /// Frees the model and any worker. The owner of the engine calls it; the listener does not.
    fn dispose(&self) {}
}

type LocalFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// The object-safe form of [`SpeechEngine`].
trait DynEngine {
    fn load<'a>(&'a self, model: ModelSize, on_progress: &'a dyn Fn(f32)) -> LocalFuture<'a, Result<(), EngineError>>;
    fn transcribe(&self, pcm: Vec<f32>, lang: Lang) -> LocalFuture<'_, Result<String, EngineError>>;
    fn begin(&self, lang: Lang);
    fn dispose(&self);
}

impl<E: SpeechEngine> DynEngine for E {
    fn load<'a>(&'a self, model: ModelSize, on_progress: &'a dyn Fn(f32)) -> LocalFuture<'a, Result<(), EngineError>> {
        Box::pin(SpeechEngine::load(self, model, on_progress))
    }

    fn transcribe(&self, pcm: Vec<f32>, lang: Lang) -> LocalFuture<'_, Result<String, EngineError>> {
        Box::pin(SpeechEngine::transcribe(self, pcm, lang))
    }

    fn begin(&self, lang: Lang) {
        SpeechEngine::begin(self, lang);
    }

    fn dispose(&self) {
        SpeechEngine::dispose(self);
    }
}

/// A shared, cheaply cloned engine for [`crate::ListenerConfig::engine`].
#[derive(Clone)]
pub struct EngineHandle(Rc<dyn DynEngine>);

impl EngineHandle {
    /// Wraps an engine.
    pub fn new(engine: impl SpeechEngine) -> Self {
        Self(Rc::new(engine))
    }

    /// See [`SpeechEngine::load`].
    ///
    /// # Errors
    ///
    /// When the engine cannot download or load the model.
    pub async fn load(&self, model: ModelSize, on_progress: &dyn Fn(f32)) -> Result<(), EngineError> {
        self.0.load(model, on_progress).await
    }

    /// See [`SpeechEngine::transcribe`].
    ///
    /// # Errors
    ///
    /// When the engine fails on the recording.
    pub async fn transcribe(&self, pcm: Vec<f32>, lang: Lang) -> Result<String, EngineError> {
        self.0.transcribe(pcm, lang).await
    }

    /// See [`SpeechEngine::begin`].
    pub fn begin(&self, lang: Lang) {
        self.0.begin(lang);
    }

    /// See [`SpeechEngine::dispose`].
    pub fn dispose(&self) {
        self.0.dispose();
    }
}

impl PartialEq for EngineHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for EngineHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EngineHandle")
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    struct Echo {
        loads: Cell<u32>,
    }

    impl SpeechEngine for Echo {
        async fn load(&self, _model: ModelSize, on_progress: &dyn Fn(f32)) -> Result<(), EngineError> {
            on_progress(1.0);
            self.loads.set(self.loads.get() + 1);
            Ok(())
        }

        async fn transcribe(&self, pcm: Vec<f32>, lang: Lang) -> Result<String, EngineError> {
            Ok(format!("{lang}:{}", pcm.len()))
        }
    }

    #[test]
    fn handle_forwards_to_the_engine() {
        let handle = EngineHandle::new(Echo { loads: Cell::new(0) });
        let seen = Cell::new(0.0);
        futures_executor::block_on(handle.load(ModelSize::Tiny, &|f| seen.set(f))).unwrap();
        assert!((seen.get() - 1.0).abs() < f32::EPSILON);
        let text = futures_executor::block_on(handle.transcribe(vec![0.0; 3], Lang::Ru)).unwrap();
        assert_eq!(text, "ru:3");
        assert_eq!(handle, handle.clone());
    }

    #[test]
    fn model_names_come_from_behavior_json() {
        assert_eq!(ModelSize::default(), ModelSize::Tiny);
        assert_eq!(ModelSize::Base.web_model_id(), "onnx-community/whisper-base");
        assert_eq!(
            ModelSize::Tiny.ggml_url(),
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin"
        );
    }

    #[test]
    fn a_base_url_gets_one_trailing_slash() {
        assert_eq!(normalize_base_url("http://localhost:5299"), "http://localhost:5299/");
        assert_eq!(normalize_base_url("http://localhost:5299/"), "http://localhost:5299/");
        assert_eq!(normalize_base_url("/models"), "/models/");
    }
}
