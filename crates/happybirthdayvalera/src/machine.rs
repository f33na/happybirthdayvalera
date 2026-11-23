//! The listener state machine from `machine.json`, as a pure transition function.

use std::fmt;
use std::str::FromStr;

use crate::i18n::Msg;

/// A listener state. [`State::as_str`] gives the `data-state` value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum State {
    /// Waiting for the user.
    #[default]
    Idle,
    /// Waiting for microphone access.
    Requesting,
    /// Downloading or loading the speech model.
    LoadingModel,
    /// Recording.
    Listening,
    /// The microphone is released; the engine works on the recording.
    Transcribing,
    /// Something failed; [`ErrorCode`] says what.
    Error,
}

impl State {
    /// Every state, in the order of `machine.json`.
    pub const ALL: [State; 6] =
        [State::Idle, State::Requesting, State::LoadingModel, State::Listening, State::Transcribing, State::Error];

    /// The contract name, for example `loading-model`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::Requesting => "requesting",
            State::LoadingModel => "loading-model",
            State::Listening => "listening",
            State::Transcribing => "transcribing",
            State::Error => "error",
        }
    }

    /// True in `requesting`, `loading-model` and `listening`: the mic button shows as pressed.
    #[must_use]
    pub const fn is_pressed(self) -> bool {
        match self {
            State::Requesting | State::LoadingModel | State::Listening => true,
            State::Idle | State::Transcribing | State::Error => false,
        }
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The error of the `FromStr` impls in this module: the name is not in `machine.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownName(pub String);

impl fmt::Display for UnknownName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` is not a name from machine.json", self.0)
    }
}

impl std::error::Error for UnknownName {}

impl FromStr for State {
    type Err = UnknownName;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        State::ALL.into_iter().find(|state| state.as_str() == s).ok_or_else(|| UnknownName(s.to_owned()))
    }
}

/// A machine event. Payloads (error codes, progress, text) travel beside the event, not in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(missing_docs, reason = "each variant is named after its machine.json event")]
pub enum Event {
    Start,
    MicReady,
    MicFailed,
    ModelProgress,
    ModelReady,
    ModelFailed,
    Stop,
    Silence,
    Result,
    TranscribeFailed,
    Reset,
}

impl Event {
    /// Every event, in the order of `machine.json`.
    pub const ALL: [Event; 11] = [
        Event::Start,
        Event::MicReady,
        Event::MicFailed,
        Event::ModelProgress,
        Event::ModelReady,
        Event::ModelFailed,
        Event::Stop,
        Event::Silence,
        Event::Result,
        Event::TranscribeFailed,
        Event::Reset,
    ];

    /// The contract name, for example `MIC_READY`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Event::Start => "START",
            Event::MicReady => "MIC_READY",
            Event::MicFailed => "MIC_FAILED",
            Event::ModelProgress => "MODEL_PROGRESS",
            Event::ModelReady => "MODEL_READY",
            Event::ModelFailed => "MODEL_FAILED",
            Event::Stop => "STOP",
            Event::Silence => "SILENCE",
            Event::Result => "RESULT",
            Event::TranscribeFailed => "TRANSCRIBE_FAILED",
            Event::Reset => "RESET",
        }
    }
}

impl FromStr for Event {
    type Err = UnknownName;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Event::ALL.into_iter().find(|event| event.as_str() == s).ok_or_else(|| UnknownName(s.to_owned()))
    }
}

/// Why the listener is in [`State::Error`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// The user or the system refused microphone access.
    MicDenied,
    /// There is no input device.
    MicUnavailable,
    /// The platform cannot capture audio or run the engine.
    Unsupported,
    /// The model did not download or load.
    ModelFailed,
    /// The engine failed on the recording.
    TranscribeFailed,
}

impl ErrorCode {
    /// Every code, in the order of `machine.json`.
    pub const ALL: [ErrorCode; 5] = [
        ErrorCode::MicDenied,
        ErrorCode::MicUnavailable,
        ErrorCode::Unsupported,
        ErrorCode::ModelFailed,
        ErrorCode::TranscribeFailed,
    ];

    /// The contract name, for example `mic-denied`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ErrorCode::MicDenied => "mic-denied",
            ErrorCode::MicUnavailable => "mic-unavailable",
            ErrorCode::Unsupported => "unsupported",
            ErrorCode::ModelFailed => "model-failed",
            ErrorCode::TranscribeFailed => "transcribe-failed",
        }
    }

    /// The UI string that the pill shows for this error.
    #[must_use]
    pub const fn msg(self) -> Msg {
        match self {
            ErrorCode::MicDenied => Msg::ErrorMicDenied,
            ErrorCode::MicUnavailable => Msg::ErrorMicUnavailable,
            ErrorCode::Unsupported => Msg::ErrorUnsupported,
            ErrorCode::ModelFailed => Msg::ErrorModelFailed,
            ErrorCode::TranscribeFailed => Msg::ErrorTranscribeFailed,
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ErrorCode {
    type Err = UnknownName;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ErrorCode::ALL.into_iter().find(|code| code.as_str() == s).ok_or_else(|| UnknownName(s.to_owned()))
    }
}

/// The next state, or `None` when the machine ignores `event` in `state`.
///
/// Every arm lists its events by name, so a new state or event fails to compile until it is placed.
#[must_use]
pub const fn transition(state: State, event: Event) -> Option<State> {
    use Event as E;
    match state {
        State::Idle => match event {
            E::Start => Some(State::Requesting),
            E::MicReady
            | E::MicFailed
            | E::ModelProgress
            | E::ModelReady
            | E::ModelFailed
            | E::Stop
            | E::Silence
            | E::Result
            | E::TranscribeFailed
            | E::Reset => None,
        },
        State::Requesting => match event {
            E::MicReady => Some(State::LoadingModel),
            E::MicFailed => Some(State::Error),
            E::Stop => Some(State::Idle),
            E::Start
            | E::ModelProgress
            | E::ModelReady
            | E::ModelFailed
            | E::Silence
            | E::Result
            | E::TranscribeFailed
            | E::Reset => None,
        },
        State::LoadingModel => match event {
            E::ModelProgress => Some(State::LoadingModel),
            E::ModelReady => Some(State::Listening),
            E::ModelFailed => Some(State::Error),
            E::Stop => Some(State::Idle),
            E::Start | E::MicReady | E::MicFailed | E::Silence | E::Result | E::TranscribeFailed | E::Reset => {
                None
            }
        },
        State::Listening => match event {
            E::Stop | E::Silence => Some(State::Transcribing),
            E::Start
            | E::MicReady
            | E::MicFailed
            | E::ModelProgress
            | E::ModelReady
            | E::ModelFailed
            | E::Result
            | E::TranscribeFailed
            | E::Reset => None,
        },
        State::Transcribing => match event {
            E::Result => Some(State::Idle),
            E::TranscribeFailed => Some(State::Error),
            E::Start
            | E::MicReady
            | E::MicFailed
            | E::ModelProgress
            | E::ModelReady
            | E::ModelFailed
            | E::Stop
            | E::Silence
            | E::Reset => None,
        },
        State::Error => match event {
            E::Start => Some(State::Requesting),
            E::Reset => Some(State::Idle),
            E::MicReady
            | E::MicFailed
            | E::ModelProgress
            | E::ModelReady
            | E::ModelFailed
            | E::Stop
            | E::Silence
            | E::Result
            | E::TranscribeFailed => None,
        },
    }
}

/// The state after `event`: the transition target, or `state` itself when the event is ignored.
#[must_use]
pub const fn next(state: State, event: Event) -> State {
    match transition(state, event) {
        Some(to) => to,
        None => state,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde::Deserialize;

    use super::*;
    use crate::shared::MACHINE_JSON;

    #[derive(Deserialize)]
    struct MachineFile {
        states: Vec<String>,
        events: Vec<String>,
        errors: Vec<String>,
        transitions: Vec<Row>,
        ignored: Vec<Row>,
    }

    #[derive(Deserialize)]
    struct Row {
        from: String,
        event: String,
        to: Option<String>,
    }

    fn machine() -> MachineFile {
        serde_json::from_str(MACHINE_JSON).expect("machine.json parses")
    }

    #[test]
    fn names_match_machine_json() {
        let m = machine();
        let names = |v: &[&'static str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(m.states, names(&State::ALL.map(State::as_str)));
        assert_eq!(m.events, names(&Event::ALL.map(Event::as_str)));
        assert_eq!(m.errors, names(&ErrorCode::ALL.map(ErrorCode::as_str)));
    }

    #[test]
    fn every_transition_row() {
        for row in machine().transitions {
            let from: State = row.from.parse().unwrap();
            let event: Event = row.event.parse().unwrap();
            let to: State = row.to.as_deref().expect("a transition row has `to`").parse().unwrap();
            assert_eq!(transition(from, event), Some(to), "{} + {}", row.from, row.event);
        }
    }

    #[test]
    fn every_ignored_row() {
        for row in machine().ignored {
            let from: State = row.from.parse().unwrap();
            let event: Event = row.event.parse().unwrap();
            assert_eq!(transition(from, event), None, "{} + {} must be ignored", row.from, row.event);
            assert_eq!(next(from, event), from);
        }
    }

    #[test]
    fn pairs_outside_the_table_leave_the_state_unchanged() {
        let table: HashMap<(State, Event), State> = machine()
            .transitions
            .iter()
            .map(|row| {
                let to = row.to.as_deref().unwrap().parse().unwrap();
                ((row.from.parse().unwrap(), row.event.parse().unwrap()), to)
            })
            .collect();
        for state in State::ALL {
            for event in Event::ALL {
                assert_eq!(transition(state, event), table.get(&(state, event)).copied(), "{state} + {event:?}");
            }
        }
    }

    #[test]
    fn pressed_states() {
        let pressed: Vec<State> = State::ALL.into_iter().filter(|s| s.is_pressed()).collect();
        assert_eq!(pressed, [State::Requesting, State::LoadingModel, State::Listening]);
    }
}
