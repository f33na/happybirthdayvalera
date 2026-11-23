//! A bottom toolbar with the voice input. Run `dx serve --platform web` or `dx serve --platform desktop`.

use std::rc::Rc;

use dioxus::prelude::*;
use happybirthdayvalera::{Lang, ListenerConfig, ListenerPill, MicButton, Theme, use_listener};

const PAGE_CSS: Asset = asset!("/assets/demo.css");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut theme = use_signal(Theme::default);
    let heard = use_signal(Vec::<(Lang, String)>::new);
    let listener = use_listener(ListenerConfig {
        easter_egg: true,
        persist_lang: true,
        theme: theme(),
        // Self-hosted models (`just serve-models`); read at build time, so set it before `dx serve`.
        model_base_url: option_env!("HBV_MODEL_BASE_URL").filter(|url| !url.is_empty()).map(str::to_owned),
        on_result: Some(Rc::new(move |text, lang| {
            // Signals are Copy; a local copy is mutable inside the shared Fn.
            let mut heard = heard;
            heard.write().push((lang, text));
        })),
        ..Default::default()
    });

    rsx! {
        document::Title { "happybirthdayvalera — Dioxus demo" }
        document::Stylesheet { href: PAGE_CSS }
        main { class: "demo",
            h1 { "Say something to Valera" }
            p { class: "demo__hint",
                "Tap the mic and speak English, Russian, German or French. Try \u{201c}happy birthday\u{201d}."
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
