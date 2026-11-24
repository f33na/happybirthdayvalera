//! A voice dictation composer built with shadcn/ui-style components and the `happybirthdayvalera` widget.
//! Run `just stand-dioxus-shadcn` and open http://localhost:8201.

mod ui;

use dioxus::prelude::*;
use happybirthdayvalera::{ListenerConfig, ListenerPill, MicButton, MicClassNames, ModelSize, PillClassNames, Theme, use_dictation};

use crate::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, CardAction, CardContent, CardDescription,
    CardFooter, CardHeader, CardTitle, Textarea,
};

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
    let mut dark = use_signal(|| false);
    let mut draft = use_signal(String::new);
    let mut sent = use_signal(Vec::<String>::new);
    // The listener writes into the textarea itself; its `input` events keep `draft` in sync.
    let listener = use_dictation(
        "message",
        ListenerConfig {
            theme: Theme::Shadcn,
            easter_egg: true,
            persist_lang: true,
            model: model(),
            model_base_url: model_base_url(),
            ..Default::default()
        },
    );

    let can_send = !draft.read().trim().is_empty();
    let send = move |_| {
        let text = draft.peek().trim().to_owned();
        if !text.is_empty() {
            sent.write().push(text);
            draft.set(String::new());
            listener.reset();
        }
    };

    rsx! {
        document::Stylesheet { href: TAILWIND }
        // An empty icon keeps the browser from asking the dev server for /favicon.ico.
        document::Link { rel: "icon", href: "data:," }
        div {
            class: if dark() { "dark" } else { "" },
            div { class: "bg-background text-foreground flex min-h-screen items-start justify-center p-6 sm:items-center",
                Card { class: "w-full max-w-xl",
                    CardHeader {
                        CardTitle { "Voice dictation" }
                        CardDescription { "Speak in English, Russian, German or French. The text lands in the message." }
                        CardAction {
                            div { class: "flex items-center gap-2",
                                Badge { variant: BadgeVariant::Outline, "{listener.lang().name()}" }
                                Button {
                                    variant: ButtonVariant::Ghost,
                                    size: ButtonSize::Icon,
                                    aria_label: "Dark mode",
                                    aria_pressed: dark(),
                                    onclick: move |_| dark.toggle(),
                                    ThemeIcon { dark: dark() }
                                }
                            }
                        }
                    }
                    CardContent { class: "flex flex-col gap-3",
                        if !sent.read().is_empty() {
                            ul { class: "flex flex-col gap-2", aria_label: "Sent messages",
                                for (i, msg) in sent.read().iter().enumerate() {
                                    li {
                                        key: "{i}",
                                        class: "bg-muted text-foreground self-end rounded-lg px-3 py-2 text-sm",
                                        "{msg}"
                                    }
                                }
                            }
                        }
                        label { class: "text-sm font-medium", r#for: "message", "Message" }
                        Textarea {
                            id: "message",
                            placeholder: "Type, or tap the mic and talk",
                            value: draft(),
                            oninput: move |e: FormEvent| draft.set(e.value()),
                        }
                    }
                    CardFooter { class: "flex-wrap gap-3",
                        MicButton {
                            listener,
                            // The widget CSS sits in `@layer components`, so a plain utility beats it.
                            class: "[--hbv-size-mic:2.25rem]",
                            class_names: MicClassNames { icon: "size-4".into(), ..Default::default() },
                        }
                        if listener.pill_visible() {
                            ListenerPill {
                                listener,
                                class: "min-w-0",
                                class_names: PillClassNames { text: "truncate".into(), ..Default::default() },
                            }
                        }
                        Button { class: "ml-auto", disabled: !can_send, onclick: send, "Send" }
                    }
                }
            }
        }
    }
}

#[component]
fn ThemeIcon(dark: bool) -> Element {
    // Lucide "moon" and "sun" outlines (ISC), the icon set shadcn/ui uses.
    rsx! {
        svg {
            "aria-hidden": "true",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            if dark {
                circle { cx: "12", cy: "12", r: "4" }
                path { d: "M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" }
            } else {
                path { d: "M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" }
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
