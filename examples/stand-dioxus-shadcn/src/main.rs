//! A voice dictation composer built with shadcn/ui-style components and the `happybirthdayvalera` widget.
//! Run `just stand-dioxus-shadcn` and open http://localhost:8201.

mod ui;

use std::rc::Rc;

use dioxus::prelude::*;
use happybirthdayvalera::{ListenerConfig, ListenerPill, MicButton, MicClassNames, PillClassNames, Theme, use_listener};

use crate::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, CardAction, CardContent, CardDescription,
    CardFooter, CardHeader, CardTitle, Textarea,
};

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
    let mut dark = use_signal(|| false);
    let mut draft = use_signal(String::new);
    let mut sent = use_signal(Vec::<String>::new);
    let listener = use_listener(ListenerConfig {
        theme: Theme::Shadcn,
        easter_egg: true,
        persist_lang: true,
        on_result: Some(Rc::new(move |text, _lang| {
            // Signals are Copy; a local copy is mutable inside the shared Fn.
            let mut draft = draft;
            let next = append(&draft.peek(), &text);
            draft.set(next);
        })),
        ..Default::default()
    });

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
                            // listener.css is not in a cascade layer, so a Tailwind utility needs `!` to beat it.
                            class: "[--hbv-size-mic:2.25rem]!",
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
    use super::append;

    #[test]
    fn append_puts_one_space_between_parts() {
        assert_eq!(append("", " Hello "), "Hello");
        assert_eq!(append("Hello  ", "world"), "Hello world");
        assert_eq!(append("   ", "world"), "world");
    }
}
