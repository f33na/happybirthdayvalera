//! Supported languages and the resolution of a preferred-language list.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::shared::{BEHAVIOR, LOCALES};

/// A recognition language. The UI language is always the recognition language.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    /// English, the fallback.
    #[default]
    En,
    /// Russian.
    Ru,
    /// German.
    De,
    /// French.
    Fr,
}

impl Lang {
    /// Every supported language, in the order of `locales.json`.
    pub const ALL: [Lang; 4] = [Lang::En, Lang::Ru, Lang::De, Lang::Fr];

    /// The code that Whisper and the contract use: `en`, `ru`, `de` or `fr`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ru => "ru",
            Lang::De => "de",
            Lang::Fr => "fr",
        }
    }

    /// Parses an exact lowercase code. Use [`resolve_preferred_lang`] for locale tags.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|lang| lang.code() == code)
    }

    /// The fallback language of `locales.json` (English).
    #[must_use]
    pub fn fallback() -> Lang {
        Lang::from_code(&LOCALES.fallback).unwrap_or_default()
    }

    /// The chip label, for example `EN`.
    #[must_use]
    pub fn short(self) -> &'static str {
        &self.entry().short
    }

    /// The language name in that language, for example `Русский`.
    #[must_use]
    pub fn name(self) -> &'static str {
        &self.entry().name
    }

    /// The language after this one in `behavior.json` → `langCycle`.
    #[must_use]
    pub fn next(self) -> Lang {
        let cycle = lang_cycle();
        let pos = cycle.iter().position(|lang| *lang == self).unwrap_or(0);
        cycle[(pos + 1) % cycle.len()]
    }

    fn entry(self) -> &'static crate::shared::LanguageEntry {
        LOCALES
            .languages
            .iter()
            .find(|entry| entry.code == self.code())
            // The `locales_list_matches_the_enum` test keeps the JSON list and this enum in step.
            .expect("every Lang has an entry in locales.json")
    }
}

impl fmt::Display for Lang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// The error of [`Lang::from_str`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownLang(pub String);

impl fmt::Display for UnknownLang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unsupported language code `{}`", self.0)
    }
}

impl std::error::Error for UnknownLang {}

impl FromStr for Lang {
    type Err = UnknownLang;

    fn from_str(code: &str) -> Result<Self, Self::Err> {
        Lang::from_code(code).ok_or_else(|| UnknownLang(code.to_owned()))
    }
}

/// The chip order from `behavior.json`. Unknown codes in the JSON are skipped; an empty cycle falls back to
/// [`Lang::ALL`].
fn lang_cycle() -> Vec<Lang> {
    let cycle: Vec<Lang> = BEHAVIOR.lang_cycle.iter().filter_map(|code| Lang::from_code(code)).collect();
    if cycle.is_empty() { Lang::ALL.to_vec() } else { cycle }
}

/// Walks the tags in order and returns the first supported primary subtag, else [`Lang::fallback`].
///
/// The primary subtag is the part before the first `-` or `_`, lowercased: `ru-RU` → `ru`, `FR_ca` → `fr`.
pub fn resolve_preferred_lang<I, S>(tags: I) -> Lang
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    tags.into_iter()
        .find_map(|tag| {
            let primary = tag.as_ref().split(['-', '_']).next().unwrap_or_default();
            Lang::from_code(&primary.to_ascii_lowercase())
        })
        .unwrap_or_else(Lang::fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_cases() {
        let cases: &[(&[&str], Lang)] = &[
            (&["ru-RU"], Lang::Ru),
            (&["pt-BR", "de"], Lang::De),
            (&["pt-BR"], Lang::En),
            (&[], Lang::En),
            (&["FR_ca"], Lang::Fr),
            (&["de-AT", "ru"], Lang::De),
            (&["EN"], Lang::En),
            (&[""], Lang::En),
            (&["zh-Hans-CN", "fr-FR"], Lang::Fr),
        ];
        for (tags, want) in cases {
            assert_eq!(resolve_preferred_lang(tags.iter()), *want, "tags {tags:?}");
        }
    }

    #[test]
    fn fallback_matches_locales_json() {
        assert_eq!(Lang::default().code(), LOCALES.fallback);
        assert_eq!(Lang::fallback(), Lang::En);
    }

    #[test]
    fn locales_list_matches_the_enum() {
        let codes: Vec<&str> = LOCALES.languages.iter().map(|entry| entry.code.as_str()).collect();
        let ours: Vec<&str> = Lang::ALL.iter().map(|lang| lang.code()).collect();
        assert_eq!(codes, ours);
    }

    #[test]
    fn cycle_follows_behavior_json() {
        assert_eq!(lang_cycle().len(), BEHAVIOR.lang_cycle.len(), "langCycle holds an unknown code");
        let mut lang = Lang::En;
        let mut seen = vec![lang];
        for _ in 1..Lang::ALL.len() {
            lang = lang.next();
            seen.push(lang);
        }
        let want: Vec<Lang> = BEHAVIOR.lang_cycle.iter().map(|code| code.parse().unwrap()).collect();
        assert_eq!(seen, want);
        assert_eq!(lang.next(), Lang::En);
    }

    #[test]
    fn short_and_name() {
        assert_eq!(Lang::Ru.short(), "RU");
        assert_eq!(Lang::Fr.name(), "Français");
        assert_eq!("de".parse::<Lang>(), Ok(Lang::De));
        assert!("pt".parse::<Lang>().is_err());
    }
}
