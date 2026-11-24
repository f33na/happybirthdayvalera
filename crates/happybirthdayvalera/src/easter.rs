//! The birthday easter egg: a final text that contains a birthday phrase in any language.

use std::sync::LazyLock;

use crate::shared::LOCALES;

/// Lowercases, folds `ё` into `е`, turns every character that is not a letter, digit or whitespace into a
/// space, and collapses whitespace.
#[must_use]
pub fn normalize(text: &str) -> String {
    let mapped: String = text
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'ё' => 'е',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every phrase of `locales.json` → `birthdayPhrases`, normalized.
static PHRASES: LazyLock<Vec<String>> = LazyLock::new(|| {
    LOCALES
        .birthday_phrases
        .iter()
        .filter(|(key, _)| !key.starts_with('$'))
        .filter_map(|(_, value)| value.as_array())
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(normalize)
        .filter(|phrase| !phrase.is_empty())
        .collect()
});

/// True when the normalized `text` contains a birthday phrase of any language.
#[must_use]
pub fn is_birthday_phrase(text: &str) -> bool {
    let normalized = normalize(text);
    PHRASES.iter().any(|phrase| normalized.contains(phrase.as_str()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct Expected {
        text: String,
    }

    #[test]
    fn normalization_rules() {
        assert_eq!(normalize("  С Днём   Рождения,\tВалера!! "), "с днем рождения валера");
        assert_eq!(normalize("Joyeux-anniversaire…"), "joyeux anniversaire");
        assert_eq!(normalize("Herzlichen Glückwunsch zum 30. Geburtstag"), "herzlichen glückwunsch zum 30 geburtstag");
        assert_eq!(normalize("ЁЖИК"), "ежик");
        assert_eq!(normalize("?!"), "");
    }

    #[test]
    fn every_fixture_phrase_triggers() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/expected.json");
        let raw = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let expected: BTreeMap<String, serde_json::Value> = serde_json::from_str(&raw).unwrap();
        let mut checked = 0;
        for (lang, value) in expected.into_iter().filter(|(key, _)| !key.starts_with('$')) {
            let entry: Expected = serde_json::from_value(value).unwrap();
            assert!(is_birthday_phrase(&entry.text), "{lang}: {}", entry.text);
            checked += 1;
        }
        assert_eq!(checked, 4);
    }

    #[test]
    fn phrases_cover_all_languages_and_ordinary_text_does_not_trigger() {
        assert!(PHRASES.len() >= 4);
        assert!(is_birthday_phrase("HAPPY BIRTHDAY!"));
        assert!(is_birthday_phrase("с днём рожденья"));
        assert!(!is_birthday_phrase("happy day, birthday later"));
        assert!(!is_birthday_phrase("The weather is lovely today."));
    }
}
