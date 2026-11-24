//! [`use_dictation`]: a listener bound to a `<textarea>` or `<input>`, so the host writes no glue code.
//!
//! The controller sends [`FieldEvent`]s; one task per binding reads the field, asks the pure
//! [`field::FieldBinding`] for the edits and applies them through the platform port: `web-sys` on the web,
//! `document::eval` elsewhere (desktop).

mod field;

#[cfg(all(feature = "web", any(target_arch = "wasm32", not(feature = "desktop"))))]
mod web;
#[cfg(all(feature = "web", any(target_arch = "wasm32", not(feature = "desktop"))))]
use web as port;

#[cfg(not(all(feature = "web", any(target_arch = "wasm32", not(feature = "desktop")))))]
mod eval;
#[cfg(not(all(feature = "web", any(target_arch = "wasm32", not(feature = "desktop")))))]
use eval as port;

#[cfg(test)]
mod tests;

use dioxus::prelude::*;
use futures_channel::mpsc::{UnboundedReceiver, unbounded};
use futures_util::StreamExt as _;

pub(crate) use field::FieldEvent;
use field::FieldBinding;

use crate::listener::{Listener, ListenerConfig, use_listener_with};

/// Creates a listener, like [`crate::use_listener`], and binds it to the `<textarea>` or `<input>` with the id
/// `target_id`. When that element is neither, the first `textarea` or `input` inside it is the field, for kits
/// that set the id of the field themselves.
///
/// - A session inserts at the caret of the field, or replaces its selection; without focus, at the end.
/// - The draft shows in the field and is replaced in place, then by the final text of the segment.
/// - One space goes before the text when the character before it is not whitespace. The first letter is
///   capitalized at the start of the field and after `.`, `!` or `?`.
/// - A user edit while a draft shows drops the draft; later segments go to the caret.
/// - Every change fires a native `input` event, so `oninput` of a controlled field sees it. A final segment is
///   inserted with `execCommand('insertText')` while the field has focus, so one undo step removes it.
/// - `Esc` in the field sends `STOP` while the mic is pressed (requesting, loading the model, listening).
/// - A keyboard press of the mic moves the focus off the field, so that session inserts at the end through
///   the value setter, without a browser undo step.
///
/// `target_id` counts only on the first render. The pill does not repeat the draft while a field is bound.
#[must_use]
#[allow(clippy::needless_pass_by_value, reason = "the same call shape as `use_listener(ListenerConfig { .. })`")]
pub fn use_dictation(target_id: &str, config: ListenerConfig) -> Listener {
    let field = use_hook(|| {
        let (tx, rx) = unbounded();
        spawn(bind(target_id.to_owned(), rx));
        tx
    });
    let listener = use_listener_with(&config, Some(&field));
    use_hook(|| {
        spawn(port::watch_escape(target_id.to_owned(), move || {
            if listener.peek_state().is_pressed() {
                listener.stop();
            }
        }))
    });
    listener
}

async fn bind(id: String, mut events: UnboundedReceiver<FieldEvent>) {
    let mut port = port::Port::new(&id);
    let mut binding = FieldBinding::default();
    while let Some(event) = events.next().await {
        let snap = port.read().await;
        for edit in binding.handle(event, snap) {
            let result = port.apply(&edit).await;
            if !binding.applied(result) {
                break;
            }
        }
    }
}
