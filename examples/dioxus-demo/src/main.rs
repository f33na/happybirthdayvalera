//! A bottom toolbar with the voice input. Run `dx serve --platform web` or `dx serve --platform desktop`.

use std::rc::Rc;

use dioxus::prelude::*;
use happybirthdayvalera::{Lang, ListenerConfig, ListenerPill, MicButton, ModelSize, Theme, use_dictation};

const PAGE_CSS: Asset = asset!("/assets/demo.css");

fn main() {
    dioxus::launch(App);
}

/// Self-hosted models (`just serve-models`); read at build time, so set it before `dx serve`.
fn model_base_url() -> Option<String> {
    option_env!("HBV_MODEL_BASE_URL").filter(|url| !url.is_empty()).map(str::to_owned)
}

/// `HBV_MODEL` (`tiny`, `base` or `small`) at build time. Without it, self-hosted models use `base`, and the
/// public hosts keep the library default.
fn model() -> ModelSize {
    model_for(option_env!("HBV_MODEL"), model_base_url().is_some())
}

fn model_for(name: Option<&str>, self_hosted: bool) -> ModelSize {
    match name.filter(|name| !name.is_empty()) {
        // A wrong name is a build setup mistake on a dev machine; failing loudly beats a silent `tiny`.
        Some(name) => name.parse().expect("HBV_MODEL is tiny, base or small"),
        None if self_hosted => ModelSize::Base,
        None => ModelSize::default(),
    }
}

#[component]
fn App() -> Element {
    let mut theme = use_signal(Theme::default);
    let heard = use_signal(Vec::<(Lang, String)>::new);
    let mut note = use_signal(String::new);
    // Drafts and final segments go straight into the textarea; the log lists every committed segment.
    let listener = use_dictation(
        "note",
        ListenerConfig {
            easter_egg: true,
            persist_lang: true,
            theme: theme(),
            model: model(),
            model_base_url: model_base_url(),
            on_result: Some(Rc::new(move |text, lang| {
                // Signals are Copy; a local copy is mutable inside the shared Fn.
                let mut heard = heard;
                heard.write().push((lang, text));
            })),
            ..Default::default()
        },
    );

    rsx! {
        document::Title { "happybirthdayvalera — Dioxus demo" }
        document::Stylesheet { href: PAGE_CSS }
        main { class: "demo",
            h1 { "Say something to Valera" }
            p { class: "demo__hint",
                "Tap the mic and speak English, Russian, German or French; hold it to talk only while pressed. "
                "Try \u{201c}happy birthday\u{201d}."
            }
            textarea {
                id: "note",
                class: "demo__note",
                rows: 4,
                placeholder: "Your words land here while you speak",
                aria_label: "Note",
                value: note(),
                oninput: move |e| note.set(e.value()),
            }
            ol { class: "demo__log", aria_live: "polite",
                for (i, (lang, text)) in heard.read().iter().enumerate() {
                    li { key: "{i}",
                        span { class: "demo__lang", "{lang.short()}" }
                        "{text}"
                    }
                }
            }
            label { class: "demo__theme",
                "Theme "
                select {
                    onchange: move |e| {
                        if let Ok(t) = e.value().parse::<Theme>() {
                            theme.set(t);
                        }
                    },
                    for t in Theme::ALL {
                        option { value: t.as_str(), selected: t == theme(), "{t}" }
                    }
                }
            }
        }
        footer { class: "demo__toolbar",
            if listener.pill_visible() {
                ListenerPill { listener }
            }
            MicButton { listener }
        }
    }
}

#[cfg(test)]
mod tests {
    use happybirthdayvalera::ModelSize;

    use super::model_for;

    #[test]
    fn self_hosted_models_default_to_base() {
        assert_eq!(model_for(None, true), ModelSize::Base);
        assert_eq!(model_for(Some(""), true), ModelSize::Base);
        assert_eq!(model_for(None, false), ModelSize::default());
        assert_eq!(model_for(Some("tiny"), true), ModelSize::Tiny);
        assert_eq!(model_for(Some("small"), false), ModelSize::Small);
    }
}
