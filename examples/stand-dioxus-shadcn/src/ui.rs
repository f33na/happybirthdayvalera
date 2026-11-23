//! Small ports of shadcn/ui components (MIT, https://ui.shadcn.com) to Dioxus. The class strings are
//! shadcn's "new-york" v4 strings; only the parts this screen uses are ported.

use dioxus::prelude::*;

/// Joins class lists; the caller's classes come last. Unlike shadcn's `cn` there is no tailwind-merge, so a
/// caller must not pass a class that conflicts with the base.
fn cn(parts: &[&str]) -> String {
    parts.iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join(" ")
}

const BUTTON_BASE: &str = "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-all disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4 shrink-0 [&_svg]:shrink-0 outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive";

/// The shadcn button variants this screen uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    #[default]
    Default,
    Ghost,
}

impl ButtonVariant {
    const fn classes(self) -> &'static str {
        match self {
            Self::Default => "bg-primary text-primary-foreground shadow-xs hover:bg-primary/90",
            Self::Ghost => "hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50",
        }
    }
}

/// The shadcn button sizes this screen uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonSize {
    #[default]
    Default,
    Icon,
}

impl ButtonSize {
    const fn classes(self) -> &'static str {
        match self {
            Self::Default => "h-9 px-4 py-2 has-[>svg]:px-3",
            Self::Icon => "size-9",
        }
    }
}

#[component]
pub fn Button(
    #[props(default)] variant: ButtonVariant,
    #[props(default)] size: ButtonSize,
    #[props(default)] class: String,
    #[props(default)] disabled: bool,
    aria_label: Option<String>,
    aria_pressed: Option<bool>,
    onclick: EventHandler<MouseEvent>,
    children: Element,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            "data-slot": "button",
            class: cn(&[BUTTON_BASE, variant.classes(), size.classes(), &class]),
            disabled,
            aria_label,
            aria_pressed: aria_pressed.map(|p| p.to_string()),
            onclick: move |e| onclick.call(e),
            {children}
        }
    }
}

#[component]
pub fn Card(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        div {
            "data-slot": "card",
            class: cn(&["bg-card text-card-foreground flex flex-col gap-6 rounded-xl border py-6 shadow-sm", &class]),
            {children}
        }
    }
}

#[component]
pub fn CardHeader(children: Element) -> Element {
    rsx! {
        div {
            "data-slot": "card-header",
            class: "@container/card-header grid auto-rows-min grid-rows-[auto_auto] items-start gap-1.5 px-6 has-data-[slot=card-action]:grid-cols-[1fr_auto] [.border-b]:pb-6",
            {children}
        }
    }
}

#[component]
pub fn CardTitle(children: Element) -> Element {
    rsx! {
        h1 { "data-slot": "card-title", class: "leading-none font-semibold", {children} }
    }
}

#[component]
pub fn CardDescription(children: Element) -> Element {
    rsx! {
        p { "data-slot": "card-description", class: "text-muted-foreground text-sm", {children} }
    }
}

#[component]
pub fn CardAction(children: Element) -> Element {
    rsx! {
        div {
            "data-slot": "card-action",
            class: "col-start-2 row-span-2 row-start-1 self-start justify-self-end",
            {children}
        }
    }
}

#[component]
pub fn CardContent(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        div { "data-slot": "card-content", class: cn(&["px-6", &class]), {children} }
    }
}

#[component]
pub fn CardFooter(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        div { "data-slot": "card-footer", class: cn(&["flex items-center px-6 [.border-t]:pt-6", &class]), {children} }
    }
}

#[component]
pub fn Textarea(
    id: String,
    #[props(default)] placeholder: String,
    value: String,
    oninput: EventHandler<FormEvent>,
) -> Element {
    rsx! {
        textarea {
            "data-slot": "textarea",
            class: "border-input placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-ring/50 aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive dark:bg-input/30 flex field-sizing-content min-h-16 w-full rounded-md border bg-transparent px-3 py-2 text-base shadow-xs transition-[color,box-shadow] outline-none focus-visible:ring-[3px] disabled:cursor-not-allowed disabled:opacity-50 md:text-sm",
            id,
            placeholder,
            rows: "5",
            value,
            oninput: move |e| oninput.call(e),
        }
    }
}

/// The shadcn badge variants this screen uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BadgeVariant {
    #[default]
    Default,
    Outline,
}

impl BadgeVariant {
    const fn classes(self) -> &'static str {
        match self {
            Self::Default => "border-transparent bg-primary text-primary-foreground [a&]:hover:bg-primary/90",
            Self::Outline => "text-foreground [a&]:hover:bg-accent [a&]:hover:text-accent-foreground",
        }
    }
}

#[component]
pub fn Badge(#[props(default)] variant: BadgeVariant, #[props(default)] class: String, children: Element) -> Element {
    rsx! {
        span {
            "data-slot": "badge",
            class: cn(&[
                "inline-flex items-center justify-center rounded-md border px-2 py-0.5 text-xs font-medium w-fit whitespace-nowrap shrink-0 [&>svg]:size-3 gap-1 [&>svg]:pointer-events-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive transition-[color,box-shadow] overflow-hidden",
                variant.classes(),
                &class,
            ]),
            {children}
        }
    }
}
