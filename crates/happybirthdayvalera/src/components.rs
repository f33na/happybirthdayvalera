//! The contract markup: `MicButton`, `ListenerPill` and `Waveform`, and the headless helpers they use.

use dioxus::prelude::*;

use crate::i18n::{Msg, format};
use crate::lang::Lang;
use crate::listener::Listener;
use crate::machine::State;
use crate::theme::{MicClassNames, PillClassNames, Theme, class_list};
use crate::wave::{amplitude, strand_path, tokens};

/// The generated stylesheet: base CSS over the design tokens plus the theme presets.
pub const STYLESHEET: Asset = asset!("/assets/listener.css");

/// Everything a host needs to render its own mic button for a listener.
///
/// Put `aria_pressed`, `aria_label`, `data_state`, `data_theme` and `data_listener` on the element, add the
/// `hbv-mic` class to get the shared CSS and the level animation, and call [`MicAttrs::click`] on click.
#[derive(Clone, Debug, PartialEq)]
pub struct MicAttrs {
    /// The listener.
    pub listener: Listener,
    /// The machine state.
    pub state: State,
    /// `"true"` in `requesting`, `loading-model` and `listening`, else `"false"`.
    pub aria_pressed: &'static str,
    /// `action.stop` while pressed, else `action.start`, in the active language.
    pub aria_label: String,
    /// The `data-state` value.
    pub data_state: &'static str,
    /// The `data-hbv-theme` value.
    pub data_theme: &'static str,
    /// The `data-hbv-listener` value that the animator uses to find the element.
    pub data_listener: String,
    /// `"true"` while the easter egg plays.
    pub data_celebrate: &'static str,
    /// The initial inline style; the animator updates `--hbv-level` per frame afterwards.
    pub style: &'static str,
}

impl MicAttrs {
    /// The click action: `STOP` while pressed, else `START`.
    pub fn click(&self) {
        self.listener.toggle();
    }
}

/// The aria, data and click props of the mic button, for hosts that render their own element.
/// `theme` overrides the listener's theme. Reading it subscribes the component to the state and language.
#[must_use]
pub fn mic_button_props(listener: Listener, theme: Option<Theme>) -> MicAttrs {
    let state = listener.state();
    let lang = listener.lang();
    let pressed = state.is_pressed();
    MicAttrs {
        listener,
        state,
        aria_pressed: if pressed { "true" } else { "false" },
        aria_label: format(lang, if pressed { Msg::ActionStop } else { Msg::ActionStart }, &[]),
        data_state: state.as_str(),
        data_theme: theme.unwrap_or_else(|| listener.theme()).as_str(),
        data_listener: listener.dom_id().to_string(),
        data_celebrate: if listener.celebrating() { "true" } else { "false" },
        style: "--hbv-level: 0",
    }
}

/// The pill's texts and flags, for hosts that compose their own pill.
#[derive(Clone, Debug, PartialEq)]
pub struct PillAttrs {
    /// The machine state, also the `data-state` value.
    pub state: State,
    /// The `data-hbv-theme` value.
    pub data_theme: &'static str,
    /// `"true"` while the easter egg plays.
    pub data_celebrate: &'static str,
    /// The group label (`label.widget`).
    pub aria_label: String,
    /// The status line.
    pub status: String,
    /// The interim text while listening or transcribing, the final text in `idle` and while celebrating;
    /// `None` when empty.
    pub text: Option<String>,
    /// The active language.
    pub lang: Lang,
    /// The chip label (`action.switchLang`).
    pub lang_aria_label: String,
    /// True when the chip is hidden because the language is forced.
    pub lang_locked: bool,
}

/// The pill's texts and flags. `theme` overrides the listener's theme.
#[must_use]
pub fn pill_props(listener: Listener, theme: Option<Theme>) -> PillAttrs {
    let state = listener.state();
    let lang = listener.lang();
    let text = match state {
        _ if listener.celebrating() => listener.text(),
        State::Listening | State::Transcribing => listener.interim(),
        State::Idle if !listener.nothing_heard() => listener.text(),
        State::Idle | State::Requesting | State::LoadingModel | State::Error => String::new(),
    };
    PillAttrs {
        state,
        data_theme: theme.unwrap_or_else(|| listener.theme()).as_str(),
        data_celebrate: if listener.celebrating() { "true" } else { "false" },
        aria_label: format(lang, Msg::LabelWidget, &[]),
        status: listener.status_text(),
        text: (!text.is_empty()).then_some(text),
        lang,
        lang_aria_label: format(lang, Msg::ActionSwitchLang, &[("lang", lang.name())]),
        lang_locked: listener.lang_locked(),
    }
}

/// The `d` attribute of strand `strand` for time `t` (seconds since mount), smoothed `level`, `state` and the
/// celebration flag, with the compiled wave tokens.
///
/// # Panics
///
/// When `strand` is not a strand index of the tokens.
#[must_use]
pub fn wave_path(strand: usize, t: f64, level: f64, state: State, celebrating: bool, reduced_motion: bool) -> String {
    let tok = tokens();
    strand_path(tok, strand, if reduced_motion { 0.0 } else { t }, amplitude(tok, state, level, celebrating, reduced_motion))
}

/// The mic button: a native `<button class="hbv-mic">` with the contract aria and data attributes.
#[component]
pub fn MicButton(
    listener: Listener,
    /// Overrides the listener's theme.
    theme: Option<Theme>,
    /// Added after `hbv-mic` on the button.
    #[props(default)]
    class: String,
    /// Per-part classes, added after the contract classes.
    #[props(default)]
    class_names: MicClassNames,
) -> Element {
    let p = mic_button_props(listener, theme);
    let click = p.clone();
    rsx! {
        document::Stylesheet { href: STYLESHEET }
        button {
            r#type: "button",
            class: class_list("hbv-mic", &[&class, &class_names.root]),
            "data-state": p.data_state,
            "data-hbv-theme": p.data_theme,
            "data-hbv-listener": p.data_listener,
            "data-celebrate": p.data_celebrate,
            aria_pressed: p.aria_pressed,
            aria_label: p.aria_label,
            style: p.style,
            onclick: move |_| click.click(),
            onmounted: move |_| listener.kick(),
            MicParts { ring_class: class_names.ring, icon_class: class_names.icon }
        }
    }
}

/// The ring and icon children of a mic button, for hosts that render their own button element.
#[component]
pub fn MicParts(#[props(default)] ring_class: String, #[props(default)] icon_class: String) -> Element {
    rsx! {
        span { class: class_list("hbv-mic__ring", &[&ring_class]), aria_hidden: "true" }
        svg {
            class: class_list("hbv-mic__icon", &[&icon_class]),
            view_box: "0 0 24 24",
            "aria-hidden": "true",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { x: "9", y: "3", width: "6", height: "11", rx: "3" }
            path { d: "M5.5 11a6.5 6.5 0 0 0 13 0" }
            path { d: "M12 17.5V21" }
        }
    }
}

/// The three-strand waveform. Its paths animate in the page; Rust renders the resting shape once.
#[component]
pub fn Waveform(
    listener: Listener,
    /// Added after `hbv-wave` on the SVG.
    #[props(default)]
    class: String,
    /// Added after `hbv-wave__strand` on every path.
    #[props(default)]
    strand_class: String,
) -> Element {
    let tok = tokens();
    // A constant first frame: the animator owns `d` from the first animation frame on, and a constant value
    // keeps Dioxus from ever writing over it on a re-render.
    let rest = tok.rest_amp;
    rsx! {
        svg {
            class: class_list("hbv-wave", &[&class]),
            view_box: "0 0 {tok.view_box_width} {tok.view_box_height}",
            "aria-hidden": "true",
            "data-hbv-listener": "{listener.dom_id()}",
            onmounted: move |_| listener.kick(),
            for (i, strand) in tok.strands.iter().enumerate() {
                path {
                    key: "{i}",
                    class: class_list("hbv-wave__strand", &[&strand_class]),
                    "data-i": "{i}",
                    d: strand_path(tok, i, 0.0, rest),
                    stroke: "var(--hbv-color-strand-{i})",
                    stroke_width: "{tok.stroke_width}",
                    opacity: "{strand.opacity}",
                    fill: "none",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                }
            }
        }
    }
}

/// The dark pill: waveform, status, dots, the optional text and the language chip.
#[component]
pub fn ListenerPill(
    listener: Listener,
    /// Overrides the listener's theme.
    theme: Option<Theme>,
    /// Added after `hbv-pill` on the group.
    #[props(default)]
    class: String,
    /// Per-part classes, added after the contract classes.
    #[props(default)]
    class_names: PillClassNames,
) -> Element {
    let p = pill_props(listener, theme);
    render_pill(&listener, p, &class, class_names, |chip| chip)
}

/// The pill markup. `wrap_chip` receives the native language button and returns what to render in its place,
/// so an adapter can put the button inside its kit's badge.
pub(crate) fn render_pill(
    listener: &Listener,
    p: PillAttrs,
    class: &str,
    class_names: PillClassNames,
    wrap_chip: impl FnOnce(Element) -> Element,
) -> Element {
    let listener = *listener;
    let chip = (!p.lang_locked).then(|| {
        wrap_chip(rsx! {
            button {
                r#type: "button",
                class: class_list("hbv-lang", &[&class_names.lang]),
                aria_label: p.lang_aria_label,
                onclick: move |_| listener.cycle_lang(),
                {p.lang.short()}
            }
        })
    });
    rsx! {
        document::Stylesheet { href: STYLESHEET }
        div {
            class: class_list("hbv-pill", &[class, &class_names.root]),
            role: "group",
            aria_label: p.aria_label,
            "data-state": p.state.as_str(),
            "data-hbv-theme": p.data_theme,
            "data-celebrate": p.data_celebrate,
            Waveform { listener, class: class_names.wave, strand_class: class_names.strand }
            span { class: class_list("hbv-pill__status", &[&class_names.status]), aria_live: "polite", {p.status} }
            if let Some(text) = p.text {
                span { class: class_list("hbv-pill__text", &[&class_names.text]), {text} }
            }
            span {
                class: class_list("hbv-dots", &[&class_names.dots]),
                aria_hidden: "true",
                i {}
                i {}
                i {}
            }
            {chip}
        }
    }
}
