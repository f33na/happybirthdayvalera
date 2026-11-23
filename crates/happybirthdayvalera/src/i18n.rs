//! UI strings from `locales.json`.

use crate::lang::Lang;
use crate::shared::LOCALES;

/// A UI string key. Each variant maps to one key of `locales.json` → `strings`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(missing_docs, reason = "each variant is named after its JSON key, which is the documentation")]
pub enum Msg {
    StatusIdle,
    StatusRequesting,
    StatusLoadingModel,
    StatusLoadingModelUnknown,
    StatusListening,
    StatusTranscribing,
    StatusNothingHeard,
    ErrorMicDenied,
    ErrorMicUnavailable,
    ErrorUnsupported,
    ErrorModelFailed,
    ErrorTranscribeFailed,
    ActionStart,
    ActionStop,
    ActionRetry,
    ActionSwitchLang,
    LabelWidget,
}

impl Msg {
    /// Every key, so a test can check that each one exists in every language.
    pub const ALL: [Msg; 17] = [
        Msg::StatusIdle,
        Msg::StatusRequesting,
        Msg::StatusLoadingModel,
        Msg::StatusLoadingModelUnknown,
        Msg::StatusListening,
        Msg::StatusTranscribing,
        Msg::StatusNothingHeard,
        Msg::ErrorMicDenied,
        Msg::ErrorMicUnavailable,
        Msg::ErrorUnsupported,
        Msg::ErrorModelFailed,
        Msg::ErrorTranscribeFailed,
        Msg::ActionStart,
        Msg::ActionStop,
        Msg::ActionRetry,
        Msg::ActionSwitchLang,
        Msg::LabelWidget,
    ];

    /// The JSON key, for example `status.loadingModel`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Msg::StatusIdle => "status.idle",
            Msg::StatusRequesting => "status.requesting",
            Msg::StatusLoadingModel => "status.loadingModel",
            Msg::StatusLoadingModelUnknown => "status.loadingModelUnknown",
            Msg::StatusListening => "status.listening",
            Msg::StatusTranscribing => "status.transcribing",
            Msg::StatusNothingHeard => "status.nothingHeard",
            Msg::ErrorMicDenied => "error.micDenied",
            Msg::ErrorMicUnavailable => "error.micUnavailable",
            Msg::ErrorUnsupported => "error.unsupported",
            Msg::ErrorModelFailed => "error.modelFailed",
            Msg::ErrorTranscribeFailed => "error.transcribeFailed",
            Msg::ActionStart => "action.start",
            Msg::ActionStop => "action.stop",
            Msg::ActionRetry => "action.retry",
            Msg::ActionSwitchLang => "action.switchLang",
            Msg::LabelWidget => "label.widget",
        }
    }
}

/// The raw string for `msg` in `lang`, with placeholders left in place.
///
/// A key missing in `lang` falls back to English; the completeness test keeps that path unused.
#[must_use]
pub fn text(lang: Lang, msg: Msg) -> &'static str {
    let lookup = |code: &str| LOCALES.strings.get(code).and_then(|table| table.get(msg.key()));
    lookup(lang.code())
        .or_else(|| lookup(Lang::En.code()))
        .map_or(msg.key(), String::as_str)
}

/// The string for `msg` in `lang` with each `{name}` placeholder replaced by its value.
#[must_use]
pub fn format(lang: Lang, msg: Msg, args: &[(&str, &str)]) -> String {
    let mut out = text(lang, msg).to_owned();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn placeholders(s: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut rest = s;
        while let Some(open) = rest.find('{') {
            let after = &rest[open + 1..];
            let Some(close) = after.find('}') else { break };
            found.insert(after[..close].to_owned());
            rest = &after[close + 1..];
        }
        found
    }

    #[test]
    fn every_language_has_exactly_the_keys_and_placeholders_of_en() {
        let en = &LOCALES.strings["en"];
        assert_eq!(LOCALES.strings.len(), Lang::ALL.len(), "a language table is missing or extra");
        for lang in Lang::ALL {
            let table = LOCALES.strings.get(lang.code()).unwrap_or_else(|| panic!("no strings for {lang}"));
            let keys: BTreeSet<&String> = table.keys().collect();
            let en_keys: BTreeSet<&String> = en.keys().collect();
            assert_eq!(keys, en_keys, "keys of {lang} differ from en");
            for (key, value) in table {
                assert_eq!(placeholders(value), placeholders(&en[key]), "placeholders of {lang}/{key}");
                assert!(!value.trim().is_empty(), "{lang}/{key} is empty");
            }
        }
    }

    #[test]
    fn the_enum_covers_exactly_the_json_keys() {
        let ours: BTreeSet<&str> = Msg::ALL.iter().map(|msg| msg.key()).collect();
        let json: BTreeSet<&str> = LOCALES.strings["en"].keys().map(String::as_str).collect();
        assert_eq!(ours, json);
    }

    #[test]
    fn format_replaces_placeholders() {
        assert_eq!(format(Lang::En, Msg::StatusLoadingModel, &[("percent", "42")]), "Loading model 42%");
        assert_eq!(
            format(Lang::Ru, Msg::ActionSwitchLang, &[("lang", Lang::Ru.name())]),
            "Язык: Русский. Сменить язык"
        );
        assert_eq!(text(Lang::De, Msg::StatusListening), "Ich höre zu");
    }
}
