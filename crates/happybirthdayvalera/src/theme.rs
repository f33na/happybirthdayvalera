//! Theme presets and per-part class overrides.

use std::fmt;
use std::str::FromStr;

/// A theme preset. Both root elements render it as `data-hbv-theme`; the generated stylesheet maps the
/// preset to the host's CSS variables.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Theme {
    /// The widget's own dark look.
    #[default]
    Default,
    /// The shadcn/ui v4 variables (`--popover`, `--ring`, …).
    Shadcn,
    /// The `@evinvest/uikit` variables.
    Evinvest,
}

impl Theme {
    /// Every preset.
    pub const ALL: [Theme; 3] = [Theme::Default, Theme::Shadcn, Theme::Evinvest];

    /// The `data-hbv-theme` value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Theme::Default => "default",
            Theme::Shadcn => "shadcn",
            Theme::Evinvest => "evinvest",
        }
    }
}

impl fmt::Display for Theme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Theme {
    type Err = crate::machine::UnknownName;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Theme::ALL.into_iter().find(|t| t.as_str() == s).ok_or_else(|| crate::machine::UnknownName(s.to_owned()))
    }
}

/// Host classes for the parts of [`crate::MicButton`]. Each one is added after the contract class.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MicClassNames {
    /// The `<button class="hbv-mic">`.
    pub root: String,
    /// The `hbv-mic__ring` span.
    pub ring: String,
    /// The `hbv-mic__icon` SVG.
    pub icon: String,
}

/// Host classes for the parts of [`crate::ListenerPill`]. Each one is added after the contract class.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PillClassNames {
    /// The `hbv-pill` group.
    pub root: String,
    /// The `hbv-wave` SVG.
    pub wave: String,
    /// Each `hbv-wave__strand` path.
    pub strand: String,
    /// The `hbv-pill__status` span.
    pub status: String,
    /// The `hbv-pill__text` span.
    pub text: String,
    /// The `hbv-dots` span.
    pub dots: String,
    /// The `hbv-lang` chip.
    pub lang: String,
}

/// `base` followed by every non-empty extra class, separated by single spaces.
#[must_use]
pub fn class_list(base: &str, extras: &[&str]) -> String {
    let mut out = base.to_owned();
    for extra in extras.iter().map(|e| e.trim()).filter(|e| !e.is_empty()) {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(extra);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_classes_come_after_the_contract_class() {
        assert_eq!(class_list("hbv-mic", &["", "  my-btn ", "ring-2"]), "hbv-mic my-btn ring-2");
        assert_eq!(class_list("hbv-pill", &[]), "hbv-pill");
    }

    #[test]
    fn names_round_trip() {
        for theme in Theme::ALL {
            assert_eq!(theme.as_str().parse::<Theme>(), Ok(theme));
        }
        assert_eq!(Theme::default().as_str(), "default");
    }
}
