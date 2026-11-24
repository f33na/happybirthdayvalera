//! The port of a bound field over `document::eval`, for the desktop webview and other renderers.

use dioxus::prelude::*;
use serde_json::json;

use super::field::{Edit, Snapshot};

const FIELD_JS: &str = include_str!("field.js");

fn script(id: &str, mode: &str) -> String {
    let id = serde_json::to_string(id).unwrap_or_else(|_| "\"\"".into());
    format!("const HBV_FIELD = {id};\nconst HBV_MODE = \"{mode}\";\n{FIELD_JS}")
}

pub(super) struct Port {
    eval: document::Eval,
}

impl Port {
    pub(super) fn new(id: &str) -> Self {
        Self { eval: document::eval(&script(id, "field")) }
    }

    pub(super) async fn read(&mut self) -> Option<Snapshot> {
        self.call(&json!({ "op": "read" })).await
    }

    pub(super) async fn apply(&mut self, edit: &Edit) -> Option<String> {
        self.call(&json!({ "op": "apply", "edit": edit })).await
    }

    async fn call<T: serde::de::DeserializeOwned>(&mut self, message: &serde_json::Value) -> Option<T> {
        if let Err(e) = self.eval.send(message) {
            tracing::debug!("the dictation field script is gone: {e}");
            return None;
        }
        match self.eval.recv::<Option<T>>().await {
            Ok(reply) => reply,
            Err(e) => {
                tracing::debug!("no reply from the dictation field script: {e}");
                None
            }
        }
    }
}

/// Calls `on_escape` for every `Esc` keydown in the field until the future is dropped.
pub(super) async fn watch_escape(id: String, on_escape: impl Fn() + 'static) {
    let mut eval = document::eval(&script(&id, "escape"));
    while eval.recv::<bool>().await.is_ok() {
        on_escape();
    }
}
