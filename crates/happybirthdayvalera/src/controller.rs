//! The controller task behind [`crate::use_listener`].
//!
//! One task per listener, scoped to the calling component, owns all mutable session state and handles one
//! message at a time: commands from the [`crate::Listener`] handle and results from short sub-tasks
//! (microphone, model, transcription, timers). Every result carries the session number of its `START`, so a
//! late result of a stopped session is dropped instead of being applied. The segmenting, the drafts and the
//! engine queue live in the pure [`Session`]; the controller runs its effects.

use dioxus::prelude::*;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures_util::StreamExt as _;
use serde_json::json;

use crate::dictation::FieldEvent;
use crate::easter::is_birthday_phrase;
use crate::engine::{EngineError, EngineHandle, ModelSize};
use crate::gesture::HoldGesture;
use crate::listener::{ListenerConfig, Signals, TextHandler};
use crate::machine::{ErrorCode, Event, State, transition};
use crate::platform::{self, Capture};
use crate::session::{Effect, Leave, Session, SessionConfig};
use crate::wave::{smooth_level, tokens};

/// The values the controller reads per message; refreshed on every render.
#[derive(Clone)]
pub(crate) struct Settings {
    pub(crate) model: ModelSize,
    pub(crate) session: SessionConfig,
    pub(crate) hold_ms: u32,
    pub(crate) easter_egg: bool,
    pub(crate) easter_egg_ms: u32,
    pub(crate) on_result: Option<TextHandler>,
    pub(crate) on_interim: Option<TextHandler>,
}

impl Settings {
    pub(crate) fn from_config(config: &ListenerConfig) -> Self {
        Self {
            model: config.model,
            session: SessionConfig {
                mode: config.mode,
                threshold_rms: config.vad_threshold_rms,
                min_speech_ms: config.min_speech_ms,
                silence_ms: config.silence_ms,
                segment_silence_ms: config.segment_silence_ms,
                auto_stop_ms: config.auto_stop_ms,
                pad_before_ms: config.segment_pad_before_ms,
                pad_after_ms: config.segment_pad_after_ms,
                max_segment_ms: config.max_segment_ms,
                max_duration_ms: config.max_duration_ms,
                interim_every_ms: config.interim_every_ms,
            },
            hold_ms: config.hold_ms,
            easter_egg: config.easter_egg,
            easter_egg_ms: config.easter_egg_ms,
            on_result: config.on_result.clone(),
            on_interim: config.on_interim.clone(),
        }
    }
}

pub(crate) enum Message {
    Start,
    Stop,
    Reset,
    Kick,
    PointerDown,
    PointerUp,
    PointerCancel,
    Click,
    HoldTimer { press: u32 },
    MicOpened { session: u32, result: Result<Capture, ErrorCode> },
    Audio { session: u32, block: Vec<f32> },
    Progress { session: u32, fraction: f32 },
    ModelLoaded { session: u32, result: Result<(), EngineError> },
    Transcribed { session: u32, job: u32, result: Result<String, EngineError> },
    CelebrationOver { round: u32 },
}

pub(crate) struct Controller {
    id: u32,
    sig: Signals,
    settings: CopyValue<Settings>,
    engine: EngineHandle,
    tx: UnboundedSender<Message>,
    field: Option<UnboundedSender<FieldEvent>>,
    session: u32,
    capture: Option<Capture>,
    core: Option<Session>,
    level: f64,
    hold: HoldGesture,
    celebration: u32,
    animator: Option<document::Eval>,
}

impl Controller {
    pub(crate) fn new(
        id: u32,
        sig: &Signals,
        settings: CopyValue<Settings>,
        engine: EngineHandle,
        tx: UnboundedSender<Message>,
        field: Option<UnboundedSender<FieldEvent>>,
    ) -> Self {
        Self {
            id,
            sig: *sig,
            settings,
            engine,
            tx,
            field,
            session: 0,
            capture: None,
            core: None,
            level: 0.0,
            hold: HoldGesture::default(),
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
            Message::PointerDown => self.pointer_down(),
            Message::PointerUp => {
                if self.hold.up() {
                    self.hold_ended();
                }
            }
            Message::PointerCancel => {
                if self.hold.cancel() {
                    self.hold_ended();
                }
            }
            Message::Click => {
                if self.hold.click() {
                    if self.state().is_pressed() { self.stop() } else { self.start() }
                }
            }
            Message::HoldTimer { press } => {
                if self.hold.timer(press) {
                    self.sig.hold.set(true);
                }
            }
            Message::MicOpened { session, result } => self.mic_opened(session, result),
            Message::Audio { session, block } => self.audio(session, &block),
            Message::Progress { session, fraction } => {
                if session == self.session && self.state() == State::LoadingModel && self.dispatch(Event::ModelProgress)
                {
                    self.sig.progress.set(Some(fraction.clamp(0.0, 1.0)));
                }
            }
            Message::ModelLoaded { session, result } => self.model_loaded(session, result),
            Message::Transcribed { session, job, result } => {
                if session == self.session
                    && let Some(core) = &mut self.core
                {
                    let fx = core.engine_done(job, result);
                    self.run_effects(fx);
                }
            }
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

    fn to_field(&self, event: FieldEvent) {
        if let Some(field) = &self.field {
            // The binding task lives as long as the component that owns this controller; a failed send means
            // that component is going away, and there is no field left to update.
            let _ = field.unbounded_send(event);
        }
    }

    fn pointer_down(&mut self) {
        let can_start = matches!(self.state(), State::Idle | State::Error);
        let Some(press) = self.hold.down(can_start) else { return };
        self.start();
        let ms = self.settings.peek().hold_ms;
        let tx = self.tx.clone();
        spawn(async move {
            platform::sleep_ms(ms).await;
            // See `to_field`: a send fails only during teardown.
            let _ = tx.unbounded_send(Message::HoldTimer { press });
        });
    }

    fn hold_ended(&mut self) {
        self.sig.hold.set(false);
        self.stop();
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
        self.core = None;
        self.to_field(FieldEvent::Start);

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
                self.to_field(FieldEvent::End);
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
                // See `to_field`: a send fails only during teardown.
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
                self.core = Some(Session::new(self.settings.peek().session));
                self.level = 0.0;
                self.dispatch(Event::ModelReady);
                self.engine.begin(*self.sig.lang.peek());
            }
            Err(e) => {
                tracing::warn!("the speech model failed: {e}");
                self.capture = None;
                self.dispatch(Event::ModelFailed);
                self.sig.error.set(Some(e.code().unwrap_or(ErrorCode::ModelFailed)));
                self.to_field(FieldEvent::End);
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
                self.to_field(FieldEvent::End);
            }
            State::Listening => {
                self.leave(Event::Stop);
                if let Some(core) = &mut self.core {
                    let fx = core.stop();
                    self.run_effects(fx);
                }
            }
            State::Idle | State::Transcribing | State::Error => {}
        }
    }

    /// Leaves `listening` and releases the microphone at once; queued segments go on in `transcribing`.
    fn leave(&mut self, event: Event) {
        if !self.dispatch(event) {
            return;
        }
        self.capture = None;
        self.level = 0.0;
        self.sig.level.set(0.0);
    }

    fn audio(&mut self, session: u32, block: &[f32]) {
        // Audio is kept only while listening.
        if session != self.session || self.state() != State::Listening {
            return;
        }
        let level = crate::vad::rms(block);
        self.level = smooth_level(tokens(), self.level, f64::from(level));
        #[allow(clippy::cast_possible_truncation, reason = "a level in 0..=1")]
        self.sig.level.set(self.level as f32);
        self.animate(&json!({ "type": "level", "rms": level }));
        if let Some(core) = &mut self.core {
            let fx = core.audio(block);
            self.run_effects(fx);
        }
    }

    fn run_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Transcribe { job, pcm } => {
                    let engine = self.engine.clone();
                    let lang = *self.sig.lang.peek();
                    let session = self.session;
                    let tx = self.tx.clone();
                    spawn(async move {
                        let result = engine.transcribe(pcm, lang).await;
                        // See `to_field`: a send fails only during teardown.
                        let _ = tx.unbounded_send(Message::Transcribed { session, job: job.id, result });
                    });
                }
                Effect::Interim { segment, text } => {
                    self.sig.interim.set(text.clone());
                    if !text.is_empty()
                        && let Some(handler) = self.settings.peek().on_interim.clone()
                    {
                        handler(text.clone(), *self.sig.lang.peek());
                    }
                    self.to_field(FieldEvent::Draft { segment, text });
                }
                Effect::Commit { segment, text } => self.commit(segment, text),
                Effect::Leave(why) => self.leave(match why {
                    Leave::Silence => Event::Silence,
                    Leave::Stop => Event::Stop,
                }),
                Effect::Done { text, failed } => self.done(&text, failed),
            }
        }
    }

    fn commit(&mut self, segment: u32, text: String) {
        let mut sig = self.sig;
        let mut all = sig.text.peek().clone();
        if !all.is_empty() {
            all.push(' ');
        }
        all.push_str(&text);
        sig.text.set(all);
        let settings = self.settings.peek().clone();
        if settings.easter_egg && is_birthday_phrase(&text) {
            self.celebrate(settings.easter_egg_ms);
        }
        self.to_field(FieldEvent::Commit { segment, text: text.clone() });
        if let Some(handler) = settings.on_result {
            handler(text, *self.sig.lang.peek());
        }
    }

    fn done(&mut self, text: &str, failed: bool) {
        self.core = None;
        let mut sig = self.sig;
        sig.interim.set(String::new());
        self.to_field(FieldEvent::End);
        if failed && text.is_empty() {
            self.dispatch(Event::TranscribeFailed);
            sig.error.set(Some(ErrorCode::TranscribeFailed));
            return;
        }
        self.dispatch(Event::Result);
        if text.is_empty() {
            sig.nothing_heard.set(true);
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
