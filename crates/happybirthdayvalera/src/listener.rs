//! `use_listener`, its config and the `Copy` handle. The hook renders nothing; the work happens in
//! [`crate::controller`].

use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use dioxus::prelude::*;
use futures_channel::mpsc::{UnboundedSender, unbounded};

use crate::controller::{Controller, Message, Settings};
use crate::engine::{EngineHandle, ModelSize, normalize_base_url};
use crate::i18n::{Msg, format};
use crate::lang::{Lang, resolve_preferred_lang};
use crate::machine::{ErrorCode, State};
use crate::platform;
use crate::shared::BEHAVIOR;
use crate::theme::Theme;
use crate::vad::VadConfig;

/// A callback with the text and the language it was recognised in.
pub type TextHandler = Rc<dyn Fn(String, Lang)>;

/// The listener settings. [`Default`] takes every value from `behavior.json`.
///
/// The hook reads the callbacks, the VAD values, `model`, `easter_egg`, `lang` and `theme` again on every
/// render; `engine`, `model_base_url`, `persist_lang` and `preload` count only on the first render.
#[derive(Clone)]
pub struct ListenerConfig {
    /// Forces the language and hides the chip.
    pub lang: Option<Lang>,
    /// Web only: remember the chosen language in `localStorage` under `behavior.json` → `persistLangKey`.
    pub persist_lang: bool,
    /// The engine; `None` uses the platform default (whisper.cpp on desktop, the Web Worker on the web).
    pub engine: Option<EngineHandle>,
    /// The Whisper model size.
    pub model: ModelSize,
    /// Self-hosted models: the default engine loads every file from this base URL instead of the Hugging Face
    /// Hub and jsDelivr. A missing trailing `/` is added. The layout under it:
    ///
    /// - `<base><model id>/…`: the transformers.js model files (web), for example
    ///   `<base>onnx-community/whisper-tiny/config.json`;
    /// - `<base>ort/`: the onnxruntime-web `.wasm` and `.mjs` files (web);
    /// - `<base>ggml-<size>.bin`: the whisper.cpp files (desktop), still checked against the pinned SHA-256.
    ///
    /// `None` keeps the public hosts. Ignored when `engine` is set.
    pub model_base_url: Option<String>,
    /// Fires once per non-empty final text.
    pub on_result: Option<TextHandler>,
    /// Fires for each interim text when `interim_every_ms > 0`.
    pub on_interim: Option<TextHandler>,
    /// Transcribe the audio so far at this interval while listening; `0` turns it off.
    pub interim_every_ms: u32,
    /// After speech, this much silence stops the recording.
    pub silence_ms: u32,
    /// This much sound at or above the threshold counts as the start of speech.
    pub min_speech_ms: u32,
    /// The RMS that counts as sound.
    pub vad_threshold_rms: f32,
    /// The recording stops at this length.
    pub max_duration_ms: u32,
    /// Celebrate a birthday phrase in the final text.
    pub easter_egg: bool,
    /// How long the celebration lasts.
    pub easter_egg_ms: u32,
    /// Load the model when the listener mounts instead of on the first `START`.
    pub preload: bool,
    /// The theme preset of the components.
    pub theme: Theme,
}

impl Default for ListenerConfig {
    fn default() -> Self {
        let vad = VadConfig::default();
        Self {
            lang: None,
            persist_lang: false,
            engine: None,
            model: ModelSize::default(),
            model_base_url: None,
            on_result: None,
            on_interim: None,
            interim_every_ms: BEHAVIOR.interim_every_ms,
            silence_ms: vad.silence_ms,
            min_speech_ms: vad.min_speech_ms,
            vad_threshold_rms: vad.threshold_rms,
            max_duration_ms: vad.max_duration_ms,
            easter_egg: false,
            easter_egg_ms: BEHAVIOR.easter_egg_ms,
            preload: false,
            theme: Theme::default(),
        }
    }
}

impl std::fmt::Debug for ListenerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListenerConfig")
            .field("lang", &self.lang)
            .field("persist_lang", &self.persist_lang)
            .field("model", &self.model)
            .field("model_base_url", &self.model_base_url)
            .field("interim_every_ms", &self.interim_every_ms)
            .field("silence_ms", &self.silence_ms)
            .field("easter_egg", &self.easter_egg)
            .field("preload", &self.preload)
            .field("theme", &self.theme)
            .finish_non_exhaustive()
    }
}

/// A `Copy` handle to a listener. Reading a getter inside a component subscribes it to that value.
#[derive(Clone, Copy, PartialEq)]
pub struct Listener {
    id: u32,
    sig: Signals,
    persist_key: CopyValue<Option<&'static str>>,
    tx: CopyValue<UnboundedSender<Message>>,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Signals {
    pub(crate) state: Signal<State>,
    pub(crate) error: Signal<Option<ErrorCode>>,
    pub(crate) progress: Signal<Option<f32>>,
    pub(crate) text: Signal<String>,
    pub(crate) interim: Signal<String>,
    pub(crate) lang: Signal<Lang>,
    pub(crate) lang_locked: Signal<bool>,
    pub(crate) nothing_heard: Signal<bool>,
    pub(crate) celebrating: Signal<bool>,
    pub(crate) level: Signal<f32>,
    pub(crate) theme: Signal<Theme>,
}

impl Listener {
    /// The machine state.
    #[must_use]
    pub fn state(&self) -> State {
        (self.sig.state)()
    }

    /// The error while the state is [`State::Error`].
    #[must_use]
    pub fn error(&self) -> Option<ErrorCode> {
        (self.sig.error)()
    }

    /// The model download or load progress, 0..=1, while the state is `loading-model`; `None` while the
    /// engine has not reported a fraction.
    #[must_use]
    pub fn progress(&self) -> Option<f32> {
        (self.sig.progress)()
    }

    /// The last non-empty final text of this session; cleared by `START`.
    #[must_use]
    pub fn text(&self) -> String {
        self.sig.text.read().clone()
    }

    /// The latest interim text while listening or transcribing.
    #[must_use]
    pub fn interim(&self) -> String {
        self.sig.interim.read().clone()
    }

    /// The active language: forced, stored or resolved from the locale.
    #[must_use]
    pub fn lang(&self) -> Lang {
        (self.sig.lang)()
    }

    /// True while `lang` is forced by the config; the language chip is hidden then.
    #[must_use]
    pub fn lang_locked(&self) -> bool {
        (self.sig.lang_locked)()
    }

    /// True when the last `RESULT` was empty, until the next `START`.
    #[must_use]
    pub fn nothing_heard(&self) -> bool {
        (self.sig.nothing_heard)()
    }

    /// True while the birthday easter egg plays.
    #[must_use]
    pub fn celebrating(&self) -> bool {
        (self.sig.celebrating)()
    }

    /// The smoothed input level 0..=1, updated once per 20 ms audio block.
    ///
    /// The built-in components do not read it: the in-page animator smooths per frame on its own.
    #[must_use]
    pub fn level(&self) -> f32 {
        (self.sig.level)()
    }

    /// True when a host should show the pill: `state != idle`, or `nothing_heard`, or `celebrating`.
    #[must_use]
    pub fn pill_visible(&self) -> bool {
        self.state() != State::Idle || self.nothing_heard() || self.celebrating()
    }

    /// The theme from the config.
    #[must_use]
    pub fn theme(&self) -> Theme {
        (self.sig.theme)()
    }

    /// The id that ties the rendered elements to this listener's animator (`data-hbv-listener`).
    #[must_use]
    pub fn dom_id(&self) -> u32 {
        self.id
    }

    /// The current status line in the active language.
    #[must_use]
    pub fn status_text(&self) -> String {
        status_text(self.lang(), self.state(), self.error(), self.progress(), self.nothing_heard())
    }

    /// Sends `START`: from `idle` or `error`, requests the microphone.
    pub fn start(&self) {
        self.send(Message::Start);
    }

    /// Sends `STOP`: cancels a start, or ends the recording and transcribes it.
    pub fn stop(&self) {
        self.send(Message::Stop);
    }

    /// `STOP` while the mic button shows as pressed, else `START`.
    pub fn toggle(&self) {
        if self.sig.state.peek().is_pressed() { self.stop() } else { self.start() }
    }

    /// Sends `RESET`: leaves `error` for `idle`.
    pub fn reset(&self) {
        self.send(Message::Reset);
    }

    /// Selects the recognition and UI language. Ignored while the language is locked.
    pub fn set_lang(&self, lang: Lang) {
        let mut sig = self.sig;
        if *sig.lang_locked.peek() || *sig.lang.peek() == lang {
            return;
        }
        sig.lang.set(lang);
        if let Some(key) = *self.persist_key.peek() {
            platform::store_lang(key, lang);
        }
    }

    /// Selects the next language of `behavior.json` → `langCycle`.
    pub fn cycle_lang(&self) {
        let next = self.sig.lang.peek().next();
        self.set_lang(next);
    }

    /// Asks the animator to look for newly mounted elements of this listener.
    pub(crate) fn kick(&self) {
        self.send(Message::Kick);
    }

    fn send(&self, message: Message) {
        // The controller task ends only when the owning component unmounts; a handle used after that has
        // nobody left to act on the message.
        let _ = self.tx.peek().unbounded_send(message);
    }
}

/// The status line for a state.
#[must_use]
pub fn status_text(
    lang: Lang,
    state: State,
    error: Option<ErrorCode>,
    progress: Option<f32>,
    nothing_heard: bool,
) -> String {
    match state {
        State::Idle if nothing_heard => format(lang, Msg::StatusNothingHeard, &[]),
        State::Idle => format(lang, Msg::StatusIdle, &[]),
        State::Requesting => format(lang, Msg::StatusRequesting, &[]),
        State::LoadingModel => match progress {
            Some(fraction) => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to 0..=100")]
                let percent = (f64::from(fraction) * 100.0).round().clamp(0.0, 100.0) as u8;
                format(lang, Msg::StatusLoadingModel, &[("percent", &percent.to_string())])
            }
            None => format(lang, Msg::StatusLoadingModelUnknown, &[]),
        },
        State::Listening => format(lang, Msg::StatusListening, &[]),
        State::Transcribing => format(lang, Msg::StatusTranscribing, &[]),
        State::Error => format(lang, error.unwrap_or(ErrorCode::Unsupported).msg(), &[]),
    }
}

static NEXT_ID: AtomicU32 = AtomicU32::new(1);

/// Creates a listener for the calling component. The hook renders nothing; pair it with [`crate::MicButton`]
/// and [`crate::ListenerPill`], or with your own markup through [`crate::mic_button_props`].
#[must_use]
#[allow(clippy::needless_pass_by_value, reason = "the contract fixes the call as `use_listener(ListenerConfig { .. })`")]
pub fn use_listener(config: ListenerConfig) -> Listener {
    let id = use_hook(|| NEXT_ID.fetch_add(1, Ordering::Relaxed));
    let persist_key = use_hook(|| CopyValue::new(config.persist_lang.then_some(BEHAVIOR.persist_lang_key.as_str())));

    let initial_lang = use_hook(|| {
        config
            .lang
            .or_else(|| persist_key.peek().and_then(platform::load_lang))
            .unwrap_or_else(|| resolve_preferred_lang(platform::preferred_locales()))
    });
    let sig = Signals {
        state: use_signal(State::default),
        error: use_signal(|| None),
        progress: use_signal(|| None),
        text: use_signal(String::new),
        interim: use_signal(String::new),
        lang: use_signal(|| initial_lang),
        lang_locked: use_signal(|| config.lang.is_some()),
        nothing_heard: use_signal(|| false),
        celebrating: use_signal(|| false),
        level: use_signal(|| 0.0),
        theme: use_signal(|| config.theme),
    };
    sync_props(sig, config.lang, config.theme);

    let mut settings = use_hook(|| CopyValue::new(Settings::from_config(&config)));
    settings.set(Settings::from_config(&config));

    let tx = use_hook(|| {
        let (tx, rx) = unbounded();
        let engine = config
            .engine
            .clone()
            .unwrap_or_else(|| platform::default_engine(config.model_base_url.as_deref().map(normalize_base_url)));
        let controller = Controller::new(id, &sig, settings, engine, tx.clone());
        // The task belongs to this component's scope; Dioxus drops it, and with it the capture, on unmount.
        spawn(controller.run(rx, config.preload));
        CopyValue::new(tx)
    });

    Listener { id, sig, persist_key, tx }
}

/// Applies a forced language or a theme that changed since the last render.
fn sync_props(mut sig: Signals, forced: Option<Lang>, theme: Theme) {
    if *sig.lang_locked.peek() != forced.is_some() {
        sig.lang_locked.set(forced.is_some());
    }
    if let Some(lang) = forced
        && *sig.lang.peek() != lang
    {
        sig.lang.set(lang);
    }
    if *sig.theme.peek() != theme {
        sig.theme.set(theme);
    }
}

impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener").field("id", &self.dom_id()).finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_lines() {
        assert_eq!(status_text(Lang::En, State::Idle, None, None, false), "Tap to speak");
        assert_eq!(status_text(Lang::En, State::Idle, None, None, true), "Didn’t catch that");
        assert_eq!(status_text(Lang::En, State::LoadingModel, None, Some(0.426), false), "Loading model 43%");
        assert_eq!(status_text(Lang::En, State::LoadingModel, None, None, false), "Loading model…");
        assert_eq!(status_text(Lang::De, State::LoadingModel, None, Some(1.0), false), "Modell wird geladen 100 %");
        assert_eq!(status_text(Lang::Ru, State::Listening, None, None, false), "Слушаю");
        assert_eq!(
            status_text(Lang::Fr, State::Error, Some(ErrorCode::MicDenied), None, false),
            "Accès au micro refusé"
        );
    }
}
