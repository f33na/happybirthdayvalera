//! Server-side renders of the components: contract markup, aria attributes, themes and class overrides.
#![cfg(not(target_arch = "wasm32"))]

use dioxus::prelude::*;
use happybirthdayvalera::{
    Lang, ListenerConfig, ListenerPill, MicButton, MicClassNames, PillClassNames, Theme, use_listener,
};

fn render(app: fn() -> Element) -> String {
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn mic_button_has_the_contract_markup_and_appends_host_classes() {
    fn app() -> Element {
        let listener = use_listener(ListenerConfig { lang: Some(Lang::En), theme: Theme::Shadcn, ..Default::default() });
        rsx! {
            MicButton {
                listener,
                class: "host-btn",
                class_names: MicClassNames { root: "root-x".into(), ring: "ring-x".into(), icon: "icon-x".into() },
            }
        }
    }
    let html = render(app);
    assert!(html.contains(r#"<button type="button" class="hbv-mic host-btn root-x""#), "{html}");
    assert!(html.contains(r#"data-state="idle""#), "{html}");
    assert!(html.contains(r#"data-hbv-theme="shadcn""#), "{html}");
    assert!(html.contains(r#"aria-pressed="false""#), "{html}");
    assert!(html.contains(r#"aria-label="Start voice input""#), "{html}");
    assert!(html.contains(r#"style="--hbv-level: 0""#), "{html}");
    assert!(html.contains(r#"data-celebrate="false""#), "the mic carries data-celebrate too: {html}");
    assert!(html.contains(r#"class="hbv-mic__ring ring-x" aria-hidden="true""#), "{html}");
    assert!(html.contains(r#"class="hbv-mic__icon icon-x""#), "{html}");
}

#[test]
fn pill_has_the_contract_markup() {
    fn app() -> Element {
        let listener = use_listener(ListenerConfig { lang: Some(Lang::Ru), ..Default::default() });
        rsx! {
            ListenerPill {
                listener,
                theme: Theme::Evinvest,
                class_names: PillClassNames { strand: "s-x".into(), status: "st-x".into(), ..Default::default() },
            }
        }
    }
    let html = render(app);
    assert!(html.contains(r#"class="hbv-pill" role="group" aria-label="Голосовой ввод""#), "{html}");
    assert!(html.contains(r#"data-state="idle""#) && html.contains(r#"data-celebrate="false""#), "{html}");
    assert!(html.contains(r#"data-hbv-theme="evinvest""#), "the per-component theme wins: {html}");
    assert!(html.contains(r#"<svg class="hbv-wave" viewBox="0 0 72 28" aria-hidden="true""#), "{html}");
    for i in 0..3 {
        assert!(html.contains(&format!(r#"class="hbv-wave__strand s-x" data-i="{i}""#)), "strand {i}: {html}");
    }
    assert!(html.contains(r#"<span class="hbv-pill__status st-x" aria-live="polite">Нажмите, чтобы говорить</span>"#));
    assert!(html.contains(r#"<span class="hbv-dots" aria-hidden="true"><i></i><i></i><i></i></span>"#), "{html}");
    assert!(!html.contains("hbv-lang"), "a forced language hides the chip: {html}");
}

#[test]
fn the_chip_shows_when_the_language_is_free() {
    fn app() -> Element {
        let listener = use_listener(ListenerConfig::default());
        rsx! {
            ListenerPill { listener }
        }
    }
    let html = render(app);
    assert!(html.contains(r#"<button type="button" class="hbv-lang" aria-label=""#), "{html}");
    assert!(html.contains(r#"data-hbv-theme="default""#), "{html}");
}

#[cfg(not(any(feature = "web", feature = "desktop")))]
#[test]
fn start_without_a_platform_ends_in_unsupported() {
    use happybirthdayvalera::State;

    fn app() -> Element {
        let listener = use_listener(ListenerConfig { lang: Some(Lang::En), ..Default::default() });
        use_hook(|| listener.start());
        let state = listener.state();
        rsx! {
            ListenerPill { listener }
            MicButton { listener }
            span { id: "done", "{state == State::Error}" }
        }
    }
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    let mut html = String::new();
    for _ in 0..20 {
        futures_executor::block_on(dom.wait_for_work());
        dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
        html = dioxus_ssr::render(&dom);
        if html.contains(r#"<span id="done">true</span>"#) {
            break;
        }
    }
    assert!(html.contains(r#"<span id="done">true</span>"#), "{html}");
    assert!(html.contains("Voice input isn’t supported here"), "{html}");
    assert!(html.contains(r#"class="hbv-pill" role="group" aria-label="Voice input" data-state="error""#), "{html}");
    assert!(html.contains(r#"aria-pressed="false""#), "{html}");
}

#[cfg(feature = "ev_uikit")]
mod ev {
    use happybirthdayvalera::ev::{EvListenerPill, EvMicButton};

    use super::*;

    #[test]
    fn ev_mic_is_a_native_button_with_kit_classes_and_aria() {
        fn app() -> Element {
            let listener = use_listener(ListenerConfig { lang: Some(Lang::De), ..Default::default() });
            rsx! {
                EvMicButton { listener, class: "host-x" }
            }
        }
        let html = render(app);
        assert!(html.contains("<button"), "{html}");
        assert!(html.contains(r#"aria-pressed="false""#), "{html}");
        assert!(html.contains(r#"aria-label="Spracheingabe starten""#), "{html}");
        assert!(html.contains(r#"data-hbv-theme="evinvest""#), "{html}");
        assert!(html.contains(r#"data-slot="button""#), "{html}");
        let class = html.split(r#"class=""#).nth(1).and_then(|rest| rest.split('"').next()).unwrap();
        assert!(class.contains("bg-secondary"), "the kit classes come first: {class}");
        assert!(class.ends_with("hbv-mic host-x"), "contract class, then host classes: {class}");
        assert!(html.contains("hbv-mic__ring") && html.contains("hbv-mic__icon"), "{html}");
    }

    #[test]
    fn ev_pill_wraps_the_chip_button_in_the_kit_badge() {
        fn app() -> Element {
            let listener = use_listener(ListenerConfig::default());
            rsx! {
                EvListenerPill { listener }
            }
        }
        let html = render(app);
        assert!(html.contains(r#"data-hbv-theme="evinvest""#), "{html}");
        assert!(html.contains(r#"data-slot="badge""#), "{html}");
        assert!(html.contains(r#"<button type="button" class="hbv-lang" aria-label=""#), "{html}");
    }
}
