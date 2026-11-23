//! Adapters over the evinvest uikit (`ev_lib::uikit_dioxus`), behind the `ev_uikit` feature.
//!
//! The adapters add no Tailwind utility of their own: Tailwind cannot scan a published crate, and a host on
//! this kit generates its CSS from `ev_lib_classes::CLASS_INVENTORY`. They use only the kit's class builder
//! and components, plus the `hbv-*` classes of the shared stylesheet. Both default to [`Theme::Evinvest`].

use dioxus::prelude::*;
use ev_lib::uikit_dioxus::{Badge, ButtonVariant, Size, Tone, button_classes};

use crate::components::{MicParts, STYLESHEET, mic_button_props, pill_props, render_pill};
use crate::listener::Listener;
use crate::theme::{MicClassNames, PillClassNames, Theme, class_list};

/// The mic as a native `<button>` that wears the kit's icon-button classes.
///
/// The kit's `Button` forwards no `aria-*` attribute, so the adapter renders the element itself with
/// `button_classes`, keeping `aria-pressed` and `aria-label`. The variant is `Primary` while pressed and
/// `Secondary` otherwise; both come from the kit.
#[component]
pub fn EvMicButton(
    listener: Listener,
    /// Overrides [`Theme::Evinvest`].
    theme: Option<Theme>,
    /// The kit size of the icon button.
    #[props(default)]
    size: Size,
    /// Added after the kit classes and `hbv-mic`.
    #[props(default)]
    class: String,
    /// Per-part classes, added after the contract classes.
    #[props(default)]
    class_names: MicClassNames,
) -> Element {
    let p = mic_button_props(listener, Some(theme.unwrap_or(Theme::Evinvest)));
    let variant = if p.state.is_pressed() { ButtonVariant::Primary } else { ButtonVariant::Secondary };
    let kit = button_classes(&variant, size, true, None, "");
    let click = p.clone();
    rsx! {
        document::Stylesheet { href: STYLESHEET }
        button {
            r#type: "button",
            class: class_list(&kit, &["hbv-mic", &class, &class_names.root]),
            "data-slot": "button",
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

/// The pill with the language chip inside the kit's `Badge`.
///
/// The Dioxus `Badge` is a `<span>` without `asChild`, so the chip stays a native `<button class="hbv-lang">`
/// inside it, with its `aria-label`. `hbv-lang--bare` drops the button's own fill, border and padding, so
/// the badge is the only box.
#[component]
pub fn EvListenerPill(
    listener: Listener,
    /// Overrides [`Theme::Evinvest`].
    theme: Option<Theme>,
    /// The kit tone of the language badge.
    #[props(default)]
    lang_tone: Tone,
    /// Added after `hbv-pill`.
    #[props(default)]
    class: String,
    /// Per-part classes, added after the contract classes.
    #[props(default)]
    mut class_names: PillClassNames,
) -> Element {
    let p = pill_props(listener, Some(theme.unwrap_or(Theme::Evinvest)));
    class_names.lang = class_list("hbv-lang--bare", &[&class_names.lang]);
    render_pill(&listener, p, &class, class_names, |chip| rsx! {
        Badge { tone: lang_tone, {chip} }
    })
}
