//! The data side of the shared contract, compiled into the crate.
//!
//! The JSON files are byte-for-byte copies of `shared/*.json` in the repository. `cargo publish` packages
//! only the crate directory, so the crate keeps its own copies and reads them at compile time.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::Deserialize;

pub(crate) const LOCALES_JSON: &str = include_str!("../shared/locales.json");
pub(crate) const BEHAVIOR_JSON: &str = include_str!("../shared/behavior.json");
pub(crate) const TOKENS_JSON: &str = include_str!("../shared/design-tokens.json");
#[cfg(test)]
pub(crate) const MACHINE_JSON: &str = include_str!("../shared/machine.json");

/// One entry of `locales.json` → `languages`.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct LanguageEntry {
    pub code: String,
    pub short: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalesFile {
    pub fallback: String,
    pub languages: Vec<LanguageEntry>,
    pub strings: BTreeMap<String, BTreeMap<String, String>>,
    /// Phrases per language; the map also holds a `$comment` string, so values stay untyped here.
    pub birthday_phrases: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopModels {
    pub base_url: String,
    pub tiny: String,
    pub base: String,
    pub small: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WebModels {
    pub tiny: String,
    pub base: String,
    pub small: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Models {
    pub default: String,
    pub web: WebModels,
    pub desktop: DesktopModels,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BehaviorFile {
    pub vad_threshold_rms: f32,
    pub min_speech_ms: u32,
    pub silence_ms: u32,
    pub max_duration_ms: u32,
    pub lang_cycle: Vec<String>,
    pub models: Models,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TokensFile {
    pub wave: crate::wave::WaveTokens,
}

// The three statics parse JSON that is compiled into the crate. A parse failure is a packaging defect that
// the unit tests catch on every build, not a runtime condition a caller could handle, so they panic.

pub(crate) static LOCALES: LazyLock<LocalesFile> =
    LazyLock::new(|| serde_json::from_str(LOCALES_JSON).expect("shared/locales.json is valid"));

pub(crate) static BEHAVIOR: LazyLock<BehaviorFile> =
    LazyLock::new(|| serde_json::from_str(BEHAVIOR_JSON).expect("shared/behavior.json is valid"));

pub(crate) static TOKENS: LazyLock<TokensFile> =
    LazyLock::new(|| serde_json::from_str(TOKENS_JSON).expect("shared/design-tokens.json is valid"));
