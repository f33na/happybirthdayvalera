//! Platform adapters: audio capture, the default engine, the locale list, storage and timers.
//!
//! Exactly one adapter is compiled. `web` wins on `wasm32`; `desktop` wins elsewhere. A native build with only
//! `web` still compiles the web adapter, so `cargo check` and `cargo publish` cover it on any host.
//!
//! Every adapter exposes the same items:
//!
//! - `open_capture(sink)`: starts the microphone and sends 16 kHz mono blocks of `BLOCK_SAMPLES` to `sink`;
//!   dropping the returned `Capture` releases the microphone at once.
//! - `default_engine(model_base_url)`: the shared engine of this thread for that normalized model host
//!   (`None`: the public hosts).
//! - `preferred_locales()`, `load_lang(key)`, `store_lang(key, lang)`.
//! - `sleep_ms(ms)`.

#[cfg(all(feature = "web", any(target_arch = "wasm32", not(feature = "desktop"))))]
mod web;
#[cfg(all(feature = "web", any(target_arch = "wasm32", not(feature = "desktop"))))]
pub(crate) use web::*;
#[cfg(all(feature = "web", any(target_arch = "wasm32", not(feature = "desktop"))))]
pub use web::WorkerEngine;

#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
mod desktop;
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) use desktop::*;
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub use desktop::{ModelStore, WhisperEngine};

#[cfg(not(any(
    all(feature = "web", any(target_arch = "wasm32", not(feature = "desktop"))),
    all(feature = "desktop", not(target_arch = "wasm32")),
)))]
mod none;
#[cfg(not(any(
    all(feature = "web", any(target_arch = "wasm32", not(feature = "desktop"))),
    all(feature = "desktop", not(target_arch = "wasm32")),
)))]
pub(crate) use none::*;

#[cfg(not(all(feature = "web", any(target_arch = "wasm32", not(feature = "desktop")))))]
mod thread_timer;
#[cfg(not(all(feature = "web", any(target_arch = "wasm32", not(feature = "desktop")))))]
pub(crate) use thread_timer::sleep_ms;
