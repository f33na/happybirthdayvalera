//! A voice dictation composer built with the evinvest Dioxus kit (`ev_lib::uikit_dioxus`) and the
//! `happybirthdayvalera` adapter for it. Run `just stand-dioxus-evinvest` and open http://localhost:8202.

use dioxus::prelude::*;
use ev_lib::uikit_dioxus::{
    Badge, Button, ButtonVariant, Card, CardAction, CardContent, CardDescription, CardFooter, CardHeader,
    CardTitle, FormControl, FormItem, FormLabel, Size, Switch, Textarea, Tone,
};
use happybirthdayvalera::ev::{EvListenerPill, EvMicButton};
use happybirthdayvalera::{ListenerConfig, ModelSize, use_dictation};

/// Written by the Tailwind step of `dx` from `tailwind.css`.
const TAILWIND: Asset = asset!("/assets/tailwind.css");

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
    // The kit is dark by default; `.light` on a wrapper switches every token under it.
    let mut light = use_signal(|| false);
    let mut draft = use_signal(String::new);
    let mut sent = use_signal(Vec::<String>::new);
    // `EvMicButton` and `EvListenerPill` default to the evinvest theme, so the config keeps the default.
    // The listener writes into the textarea itself; its `input` events keep `draft` in sync. The kit's
    // `FormItem` owns the textarea id, so the binding names a wrapper and finds the textarea inside it.
    let listener = use_dictation(
        "message",
        ListenerConfig { easter_egg: true, persist_lang: true, model: model(), model_base_url: model_base_url(), ..Default::default() },
    );

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
                                div { id: "message",
                                    Textarea {
                                        placeholder: "Type, or tap the mic and talk",
                                        rows: 5,
                                        value: draft(),
                                        oninput: move |e: FormEvent| draft.set(e.value()),
                                    }
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
