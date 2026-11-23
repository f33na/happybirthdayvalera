//! A voice dictation composer built with the evinvest Dioxus kit (`ev_lib::uikit_dioxus`) and the
//! `happybirthdayvalera` adapter for it. Run `just stand-dioxus-evinvest` and open http://localhost:8202.

use std::rc::Rc;

use dioxus::prelude::*;
use ev_lib::uikit_dioxus::{
    Badge, Button, ButtonVariant, Card, CardAction, CardContent, CardDescription, CardFooter, CardHeader,
    CardTitle, FormControl, FormItem, FormLabel, Size, Switch, Textarea, Tone,
};
use happybirthdayvalera::ListenerConfig;
use happybirthdayvalera::ev::{EvListenerPill, EvMicButton};
use happybirthdayvalera::use_listener;

/// Written by the Tailwind step of `dx` from `tailwind.css`.
const TAILWIND: Asset = asset!("/assets/tailwind.css");

fn main() {
    dioxus::launch(App);
}

/// Appends a transcript to the draft with one space between them.
fn append(draft: &str, text: &str) -> String {
    let text = text.trim();
    if draft.trim_end().is_empty() {
        text.to_owned()
    } else {
        format!("{} {text}", draft.trim_end())
    }
}

#[component]
fn App() -> Element {
    // The kit is dark by default; `.light` on a wrapper switches every token under it.
    let mut light = use_signal(|| false);
    let mut draft = use_signal(String::new);
    let mut sent = use_signal(Vec::<String>::new);
    // `EvMicButton` and `EvListenerPill` default to the evinvest theme, so the config keeps the default.
    let listener = use_listener(ListenerConfig {
        easter_egg: true,
        persist_lang: true,
        // Self-hosted models (`just serve-models`); read at build time, so set it before `dx serve`.
        model_base_url: option_env!("HBV_MODEL_BASE_URL").filter(|url| !url.is_empty()).map(str::to_owned),
        on_result: Some(Rc::new(move |text, _lang| {
            // Signals are Copy; a local copy is mutable inside the shared Fn.
            let mut draft = draft;
            let next = append(&draft.peek(), &text);
            draft.set(next);
        })),
        ..Default::default()
    });

    let can_send = !draft.read().trim().is_empty();

    rsx! {
        document::Stylesheet { href: TAILWIND }
        // An empty icon keeps the browser from asking the dev server for /favicon.ico.
        document::Link { rel: "icon", href: "data:," }
        div {
            class: if light() { "light" } else { "" },
            div { class: "bg-background text-ink flex min-h-screen items-start justify-center p-6 font-sans sm:items-center",
                Card { class: "w-full max-w-xl",
                    CardHeader {
                        CardTitle { "Voice dictation" }
                        CardDescription { "Speak in English, Russian, German or French. The text lands in the message." }
                        CardAction {
                            div { class: "flex items-center gap-3",
                                Badge { tone: Tone::Primary, "{listener.lang().name()}" }
                                label { class: "text-ink-mid text-body-s flex items-center gap-2",
                                    Switch { checked: light(), on_checked_change: move |on| light.set(on) }
                                    "Light"
                                }
                            }
                        }
                    }
                    CardContent { class: "flex flex-col gap-3",
                        if !sent.read().is_empty() {
                            ul { class: "flex flex-col gap-2", aria_label: "Sent messages",
                                for (i, msg) in sent.read().iter().enumerate() {
                                    li { key: "{i}", class: "self-end",
                                        Badge { tone: Tone::Neutral, class: "whitespace-normal text-left", "{msg}" }
                                    }
                                }
                            }
                        }
                        FormItem {
                            FormLabel { "Message" }
                            FormControl {
                                Textarea {
                                    placeholder: "Type, or tap the mic and talk",
                                    rows: 5,
                                    value: draft(),
                                    oninput: move |e: FormEvent| draft.set(e.value()),
                                }
                            }
                        }
                    }
                    CardFooter { class: "flex-wrap gap-3",
                        EvMicButton { listener, size: Size::Md }
                        if listener.pill_visible() {
                            EvListenerPill { listener, lang_tone: Tone::Neutral, class: "min-w-0" }
                        }
                        Button {
                            variant: ButtonVariant::Primary,
                            class: "ml-auto",
                            r#type: "button",
                            disabled: !can_send,
                            onclick: move |_| {
                                let text = draft.peek().trim().to_owned();
                                if !text.is_empty() {
                                    sent.write().push(text);
                                    draft.set(String::new());
                                    listener.reset();
                                }
                            },
                            "Send"
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::append;

    #[test]
    fn append_puts_one_space_between_parts() {
        assert_eq!(append("", " Hello "), "Hello");
        assert_eq!(append("Hello  ", "world"), "Hello world");
        assert_eq!(append("   ", "world"), "world");
    }
}
