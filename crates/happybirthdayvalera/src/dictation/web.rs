//! The browser port of a bound field, over `web-sys`.

use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::Closure;
use web_sys::{Document, HtmlElement, HtmlInputElement, HtmlTextAreaElement, KeyboardEvent};

use super::field::{Edit, Snapshot, len16};

enum Field {
    Area(HtmlTextAreaElement),
    Input(HtmlInputElement),
}

fn document() -> Option<Document> {
    web_sys::window()?.document()
}

impl Field {
    fn find(id: &str) -> Option<Self> {
        let mut el = document()?.get_element_by_id(id)?;
        if !matches!(el.tag_name().as_str(), "TEXTAREA" | "INPUT") {
            // A query error means an invalid selector, which this constant is not; no field then.
            el = el.query_selector("textarea, input").ok().flatten()?;
        }
        match el.dyn_into::<HtmlTextAreaElement>() {
            Ok(area) => Some(Field::Area(area)),
            Err(el) => el.dyn_into::<HtmlInputElement>().ok().map(Field::Input),
        }
    }

    fn element(&self) -> &HtmlElement {
        match self {
            Field::Area(el) => el,
            Field::Input(el) => el,
        }
    }

    fn value(&self) -> String {
        match self {
            Field::Area(el) => el.value(),
            Field::Input(el) => el.value(),
        }
    }

    fn set_value(&self, value: &str) {
        match self {
            Field::Area(el) => el.set_value(value),
            Field::Input(el) => el.set_value(value),
        }
    }

    /// `None` for input types without a selection, such as `email`.
    fn selection(&self) -> Option<(u32, u32)> {
        let (start, end) = match self {
            Field::Area(el) => (el.selection_start(), el.selection_end()),
            Field::Input(el) => (el.selection_start(), el.selection_end()),
        };
        Some((start.ok().flatten()?, end.ok().flatten()?))
    }

    fn select(&self, start: usize, end: usize) {
        let (Ok(start), Ok(end)) = (u32::try_from(start), u32::try_from(end)) else { return };
        let result = match self {
            Field::Area(el) => el.set_selection_range(start, end),
            Field::Input(el) => el.set_selection_range(start, end),
        };
        if let Err(e) = result {
            // Input types without a selection throw here; the text is in place all the same.
            tracing::debug!("cannot place the caret in the dictation field: {e:?}");
        }
    }

    fn focused(&self) -> bool {
        document().and_then(|d| d.active_element()).is_some_and(|active| &active == self.element().as_ref())
    }
}

pub(super) struct Port {
    id: String,
}

impl Port {
    pub(super) fn new(id: &str) -> Self {
        Self { id: id.to_owned() }
    }

    #[allow(clippy::unused_async, reason = "the same signature as the async desktop port")]
    pub(super) async fn read(&mut self) -> Option<Snapshot> {
        let field = Field::find(&self.id)?;
        let value = field.value();
        let len = u32::try_from(len16(&value)).unwrap_or(u32::MAX);
        let (start, end) = field.selection().unwrap_or((len, len));
        Some(Snapshot { value, sel_start: start as usize, sel_end: end as usize, focused: field.focused() })
    }

    #[allow(clippy::unused_async, reason = "the same signature as the async desktop port")]
    pub(super) async fn apply(&mut self, edit: &Edit) -> Option<String> {
        let field = Field::find(&self.id)?;
        if field.value() != edit.expect {
            return None;
        }
        if edit.commit && field.focused() && insert_text(&field, edit) {
            return Some(field.value());
        }
        field.set_value(&edit.after);
        field.select(edit.caret, edit.caret);
        fire_input(field.element());
        Some(field.value())
    }
}

/// Inserts a final segment through the browser's editing command, which puts it on the undo stack and fires
/// `input` itself. False when the browser refused; the caller then uses the value setter.
fn insert_text(field: &Field, edit: &Edit) -> bool {
    let Some(doc) = document().and_then(|d| d.dyn_into::<web_sys::HtmlDocument>().ok()) else { return false };
    field.select(edit.start, edit.end);
    match doc.exec_command_with_show_ui_and_value("insertText", false, &edit.text) {
        // The caller records the value the field now holds, so a browser that normalises the text is fine.
        Ok(true) => true,
        Ok(false) => false,
        Err(e) => {
            tracing::debug!("execCommand('insertText') failed: {e:?}");
            false
        }
    }
}

fn fire_input(el: &HtmlElement) {
    let init = web_sys::EventInit::new();
    init.set_bubbles(true);
    match web_sys::Event::new_with_event_init_dict("input", &init) {
        Ok(event) => {
            if let Err(e) = el.dispatch_event(&event) {
                tracing::warn!("cannot fire input on the dictation field: {e:?}");
            }
        }
        Err(e) => tracing::warn!("cannot create an input event: {e:?}"),
    }
}

/// Calls `on_escape` for every `Esc` keydown in the field until the future is dropped.
pub(super) async fn watch_escape(id: String, on_escape: impl Fn() + 'static) {
    struct Listener {
        doc: Document,
        callback: Closure<dyn FnMut(KeyboardEvent)>,
    }
    impl Drop for Listener {
        fn drop(&mut self) {
            if let Err(e) =
                self.doc.remove_event_listener_with_callback("keydown", self.callback.as_ref().unchecked_ref())
            {
                tracing::debug!("cannot remove the Esc listener: {e:?}");
            }
        }
    }

    let Some(doc) = document() else { return };
    let callback = Closure::<dyn FnMut(KeyboardEvent)>::new(move |e: KeyboardEvent| {
        if e.key() != "Escape" {
            return;
        }
        let Some(field) = Field::find(&id) else { return };
        if e.target().is_some_and(|target| &target == field.element().as_ref()) {
            on_escape();
        }
    });
    if let Err(e) = doc.add_event_listener_with_callback("keydown", callback.as_ref().unchecked_ref()) {
        tracing::warn!("cannot listen for Esc in the dictation field: {e:?}");
        return;
    }
    let _listener = Listener { doc, callback };
    std::future::pending::<()>().await;
}
