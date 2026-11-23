//! The controller task behind [`crate::use_listener`].
//!
//! One task per listener, scoped to the calling component, owns all mutable session state and handles one
//! message at a time: commands from the [`crate::Listener`] handle and results from short sub-tasks
//! (microphone, model, transcription, timers). Every result carries the session number of its `START`, so a
//! late result of a stopped session is dropped instead of being applied.

use dioxus::prelude::*;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures_util::StreamExt as _;
use serde_json::json;

use crate::easter::is_birthday_phrase;
use crate::engine::{EngineError, EngineHandle, ModelSize};
use crate::listener::{ListenerConfig, Signals, TextHandler};
use crate::machine::{ErrorCode, Event, State, transition};
use crate::platform::{self, Capture};
use crate::resample::TARGET_RATE;
use crate::vad::{Vad, VadConfig, VadEvent, rms};
use crate::wave::{smooth_level, tokens};

/// The values the controller reads per message; refreshed on every render.
#[derive(Clone)]
pub(crate) struct Settings {
    pub(crate) model: ModelSize,
    pub(crate) vad: VadConfig,
    pub(crate) interim_every_ms: u32,
    pub(crate) easter_egg: bool,
    pub(crate) easter_egg_ms: u32,
    pub(crate) on_result: Option<TextHandler>,
    pub(crate) on_interim: Option<TextHandler>,
}

impl Settings {
    pub(crate) fn from_config(config: &ListenerConfig) -> Self {
        Self {
            model: config.model,
            vad: VadConfig {
                threshold_rms: config.vad_threshold_rms,
                min_speech_ms: config.min_speech_ms,
                silence_ms: config.silence_ms,
                max_duration_ms: config.max_duration_ms,
            },
            interim_every_ms: config.interim_every_ms,
            easter_egg: config.easter_egg,
            easter_egg_ms: config.easter_egg_ms,
            on_result: config.on_result.clone(),
            on_interim: config.on_interim.clone(),
        }
    }
}

/// Recordings shorter than this skip the engine: Whisper invents text from such short audio.
pub(crate) const MIN_TRANSCRIBE_SAMPLES: usize = TARGET_RATE as usize * 3 / 10;

pub(crate) enum Message {
    Start,
    Stop,
    Reset,
    Kick,
    MicOpened { session: u32, result: Result<Capture, ErrorCode> },
    Audio { session: u32, block: Vec<f32> },
    Progress { session: u32, fraction: f32 },
    ModelLoaded { session: u32, result: Result<(), EngineError> },
    Transcribed { session: u32, result: Result<String, EngineError> },
    Interim { session: u32, result: Result<String, EngineError> },
    CelebrationOver { round: u32 },
}

pub(crate) struct Controller {
    id: u32,
    sig: Signals,
    settings: CopyValue<Settings>,
    engine: EngineHandle,
    tx: UnboundedSender<Message>,
    session: u32,
    capture: Option<Capture>,
    pcm: Vec<f32>,
    vad: Vad,
    level: f64,
    since_interim: usize,
    interim_busy: bool,
    celebration: u32,
    animator: Option<document::Eval>,
}

impl Controller {
    pub(crate) fn new(id: u32, sig: &Signals, settings: CopyValue<Settings>, engine: EngineHandle, tx: UnboundedSender<Message>) -> Self {
        let vad = Vad::new(settings.peek().vad);
        Self {
            id,
            sig: *sig,
            settings,
            engine,
            tx,
            session: 0,
            capture: None,
            pcm: Vec::new(),
            vad,
            level: 0.0,
            since_interim: 0,
            interim_busy: false,
            celebration: 0,
            animator: None,
        }
    }

    pub(crate) async fn run(mut self, mut rx: UnboundedReceiver<Message>, preload: bool) {
        self.animator = Some(document::eval(&animator_script(self.id)));
        self.animate(&json!({ "type": "state", "state": State::Idle.as_str() }));
        if preload {
            let engine = self.engine.clone();
            let model = self.settings.peek().model;
            spawn(async move {
                if let Err(e) = engine.load(model, &|_| {}).await {
                    tracing::warn!("preloading the {model} model failed: {e}");
                }
            });
        }
        while let Some(message) = rx.next().await {
            self.handle(message);
        }
    }

    fn handle(&mut self, message: Message) {
        match message {
            Message::Start => self.start(),
            Message::Stop => self.stop(),
            Message::Reset => {
                if self.dispatch(Event::Reset) {
                    self.sig.error.set(None);
                }
            }
            Message::Kick => self.animate(&json!({ "type": "kick" })),
            Message::MicOpened { session, result } => self.mic_opened(session, result),
            Message::Audio { session, block } => self.audio(session, &block),
            Message::Progress { session, fraction } => {
                if session == self.session && self.state() == State::LoadingModel && self.dispatch(Event::ModelProgress)
                {
                    self.sig.progress.set(Some(fraction.clamp(0.0, 1.0)));
                }
            }
            Message::ModelLoaded { session, result } => self.model_loaded(session, result),
            Message::Transcribed { session, result } => self.transcribed(session, result),
            Message::Interim { session, result } => self.interim(session, result),
            Message::CelebrationOver { round } => {
                if round == self.celebration {
                    self.sig.celebrating.set(false);
                    self.animate(&json!({ "type": "celebrate", "on": false }));
                }
            }
        }
    }

    fn state(&self) -> State {
        *self.sig.state.peek()
    }

    /// Applies `event`; returns false when the machine ignores it.
    fn dispatch(&mut self, event: Event) -> bool {
        let from = self.state();
        let Some(to) = transition(from, event) else { return false };
        if to != from {
            self.sig.state.set(to);
            self.animate(&json!({ "type": "state", "state": to.as_str() }));
        }
        true
    }

    fn animate(&self, message: &serde_json::Value) {
        if let Some(animator) = &self.animator
            && let Err(e) = animator.send(message)
        {
            // No document (server rendering, tests) or a page that went away: there is nothing to animate.
            tracing::trace!("animator message dropped: {e}");
        }
    }

    fn post(&self, message: Message) {
        // The receiver lives as long as this controller, so a send fails only during teardown.
        let _ = self.tx.unbounded_send(message);
    }

    fn start(&mut self) {
        if !self.dispatch(Event::Start) {
            return;
        }
        self.session = self.session.wrapping_add(1);
        let mut sig = self.sig;
        sig.error.set(None);
        sig.nothing_heard.set(false);
        sig.text.set(String::new());
        sig.interim.set(String::new());
        sig.progress.set(None);
        // A new START ends a celebration that is still playing.
        self.celebration = self.celebration.wrapping_add(1);
        if *sig.celebrating.peek() {
            sig.celebrating.set(false);
            self.animate(&json!({ "type": "celebrate", "on": false }));
        }
        self.pcm.clear();

        let session = self.session;
        let (blocks_tx, mut blocks_rx) = unbounded::<Vec<f32>>();
        let tx = self.tx.clone();
        spawn(async move {
            let result = platform::open_capture(blocks_tx).await;
            // The listener may be gone by now; dropping the message also drops (releases) the capture.
            let _ = tx.unbounded_send(Message::MicOpened { session, result });
        });
        let tx = self.tx.clone();
        spawn(async move {
            // Ends when the capture drops its sender.
            while let Some(block) = blocks_rx.next().await {
                if tx.unbounded_send(Message::Audio { session, block }).is_err() {
                    break;
                }
            }
        });
    }

    fn mic_opened(&mut self, session: u32, result: Result<Capture, ErrorCode>) {
        if session != self.session || self.state() != State::Requesting {
            return; // stale: dropping `result` releases a late microphone
        }
        match result {
            Ok(capture) => {
                self.capture = Some(capture);
                self.dispatch(Event::MicReady);
                self.load_model();
            }
            Err(code) => {
                self.dispatch(Event::MicFailed);
                self.sig.error.set(Some(code));
            }
        }
    }

    fn load_model(&self) {
        let session = self.session;
        let engine = self.engine.clone();
        let model = self.settings.peek().model;
        let tx = self.tx.clone();
        spawn(async move {
            let progress_tx = tx.clone();
            let on_progress = move |fraction: f32| {
                // See `post`: a send fails only during teardown.
                let _ = progress_tx.unbounded_send(Message::Progress { session, fraction });
            };
            let result = engine.load(model, &on_progress).await;
            let _ = tx.unbounded_send(Message::ModelLoaded { session, result });
        });
    }

    fn model_loaded(&mut self, session: u32, result: Result<(), EngineError>) {
        if session != self.session || self.state() != State::LoadingModel {
            return;
        }
        match result {
            Ok(()) => {
                self.vad = Vad::new(self.settings.peek().vad);
                self.pcm.clear();
                self.level = 0.0;
                self.since_interim = 0;
                self.interim_busy = false;
                self.dispatch(Event::ModelReady);
                self.engine.begin(*self.sig.lang.peek());
            }
            Err(e) => {
                tracing::warn!("the speech model failed: {e}");
                self.capture = None;
                self.dispatch(Event::ModelFailed);
                self.sig.error.set(Some(e.code().unwrap_or(ErrorCode::ModelFailed)));
            }
        }
    }

    fn stop(&mut self) {
        match self.state() {
            State::Requesting | State::LoadingModel => {
                self.dispatch(Event::Stop);
                self.capture = None;
                // Results of the cancelled start must not apply to the next session.
                self.session = self.session.wrapping_add(1);
            }
            State::Listening => self.finish(Event::Stop),
            State::Idle | State::Transcribing | State::Error => {}
        }
    }

    /// Leaves `listening`: releases the microphone at once, then transcribes.
    fn finish(&mut self, event: Event) {
        if !self.dispatch(event) {
            return;
        }
        self.capture = None;
        self.level = 0.0;
        self.sig.level.set(0.0);
        let pcm = std::mem::take(&mut self.pcm);
        let session = self.session;
        if !self.vad.heard_speech() || pcm.len() < MIN_TRANSCRIBE_SAMPLES {
            self.post(Message::Transcribed { session, result: Ok(String::new()) });
            return;
        }
        let engine = self.engine.clone();
        let lang = *self.sig.lang.peek();
        let tx = self.tx.clone();
        spawn(async move {
            let result = engine.transcribe(pcm, lang).await;
            // See `post`: a send fails only during teardown.
            let _ = tx.unbounded_send(Message::Transcribed { session, result });
        });
    }

    fn audio(&mut self, session: u32, block: &[f32]) {
        // Audio is kept only while listening.
        if session != self.session || self.state() != State::Listening {
            return;
        }
        let level = rms(block);
        self.pcm.extend_from_slice(block);
        self.level = smooth_level(tokens(), self.level, f64::from(level));
        #[allow(clippy::cast_possible_truncation, reason = "a level in 0..=1")]
        self.sig.level.set(self.level as f32);
        self.animate(&json!({ "type": "level", "rms": level }));

        #[allow(clippy::cast_precision_loss, reason = "a block holds a few hundred samples")]
        let block_ms = block.len() as f64 * 1000.0 / f64::from(TARGET_RATE);
        match self.vad.push(level, block_ms) {
            Some(VadEvent::Silence) => return self.finish(Event::Silence),
            Some(VadEvent::MaxDuration) => return self.finish(Event::Stop),
            Some(VadEvent::SpeechStarted) | None => {}
        }
        self.maybe_interim(block.len());
    }

    fn maybe_interim(&mut self, added: usize) {
        let every_ms = self.settings.peek().interim_every_ms;
        if every_ms == 0 {
            return;
        }
        self.since_interim += added;
        let every = TARGET_RATE as usize * every_ms as usize / 1000;
        if self.since_interim < every {
            return;
        }
        self.since_interim = 0;
        // Skip the tick while the previous interim call runs, and before there is speech to transcribe.
        if self.interim_busy || !self.vad.heard_speech() || self.pcm.len() < MIN_TRANSCRIBE_SAMPLES {
            return;
        }
        self.interim_busy = true;
        let engine = self.engine.clone();
        let pcm = self.pcm.clone();
        let lang = *self.sig.lang.peek();
        let session = self.session;
        let tx = self.tx.clone();
        spawn(async move {
            let result = engine.transcribe(pcm, lang).await;
            let _ = tx.unbounded_send(Message::Interim { session, result });
        });
    }

    fn interim(&mut self, session: u32, result: Result<String, EngineError>) {
        if session != self.session {
            return;
        }
        self.interim_busy = false;
        if !matches!(self.state(), State::Listening | State::Transcribing) {
            return;
        }
        match result {
            Ok(text) => {
                let text = text.trim().to_owned();
                if text.is_empty() {
                    return;
                }
                self.sig.interim.set(text.clone());
                let handler = self.settings.peek().on_interim.clone();
                if let Some(handler) = handler {
                    handler(text, *self.sig.lang.peek());
                }
            }
            Err(e) => tracing::debug!("an interim transcription failed: {e}"),
        }
    }

    fn transcribed(&mut self, session: u32, result: Result<String, EngineError>) {
        if session != self.session || self.state() != State::Transcribing {
            return;
        }
        match result {
            Ok(text) => {
                let text = text.trim().to_owned();
                self.dispatch(Event::Result);
                let mut sig = self.sig;
                sig.interim.set(String::new());
                if text.is_empty() {
                    sig.nothing_heard.set(true);
                    return;
                }
                sig.text.set(text.clone());
                let settings = self.settings.peek().clone();
                if settings.easter_egg && is_birthday_phrase(&text) {
                    self.celebrate(settings.easter_egg_ms);
                }
                if let Some(handler) = settings.on_result {
                    handler(text, *self.sig.lang.peek());
                }
            }
            Err(e) => {
                tracing::warn!("transcription failed: {e}");
                self.dispatch(Event::TranscribeFailed);
                self.sig.error.set(Some(ErrorCode::TranscribeFailed));
            }
        }
    }

    fn celebrate(&mut self, ms: u32) {
        self.celebration = self.celebration.wrapping_add(1);
        self.sig.celebrating.set(true);
        self.animate(&json!({ "type": "celebrate", "on": true }));
        let round = self.celebration;
        let tx = self.tx.clone();
        spawn(async move {
            platform::sleep_ms(ms).await;
            let _ = tx.unbounded_send(Message::CelebrationOver { round });
        });
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.animate(&json!({ "type": "stop" }));
    }
}

const ANIMATOR_JS: &str = include_str!("animator.js");

/// The animator for listener `id`: the shared JS with this listener's id and the wave tokens.
pub(crate) fn animator_script(id: u32) -> String {
    let tokens = serde_json::to_string(tokens()).unwrap_or_else(|_| "null".into());
    format!("const HBV_ID = {id};\nconst HBV_TOK = {tokens};\n{ANIMATOR_JS}\nawait hbvRun(dioxus, HBV_ID, HBV_TOK);\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animator_script_embeds_the_tokens() {
        let script = animator_script(7);
        assert!(script.starts_with("const HBV_ID = 7;\nconst HBV_TOK = {\"viewBoxWidth\":72.0"));
        assert!(script.contains("async function hbvRun"));
    }
}
