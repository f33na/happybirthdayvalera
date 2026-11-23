# Stand: Dioxus + evinvest kit

A small "voice dictation composer" screen on the evinvest Dioxus kit (`ev_lib::uikit_dioxus`):

- a kit `Card` with a title and a kit `Badge` with the active language,
- a kit `Textarea` in a `FormItem`, which each final transcript appends to,
- a toolbar row with `EvMicButton`, `EvListenerPill` while the pill is visible, and a kit `Button` to send,
- a kit `Switch` for light mode. The kit is dark by default, and `.light` on a wrapper switches it.

## Run

```sh
just stand-dioxus-evinvest      # dx serve on http://localhost:8202
just stands-dioxus-build        # dx build of both stands, then clippy
```

`ev_lib` 0.34 and `ev_lib_classes` 0.20 turn on the nightly-only `#![feature(default_field_values)]`,
so the `ev_uikit` feature of the crate needs a nightly toolchain or `RUSTC_BOOTSTRAP=1`. The recipes set
`RUSTC_BOOTSTRAP=1`, so the stable toolchain builds them. Without it, the build fails with E0554. This
stays so until EV-Main/web_std drops that feature from both crates. The real `listener.css` and `whisper-worker.js` come from `just sync-assets`.

## What it proves

- The `ev_uikit` feature of the crate works in a real host: `EvMicButton` wears the kit's
  `button_classes`, and `EvListenerPill` puts the language chip inside the kit's `Badge`. Both default to
  `Theme::Evinvest`, which maps the `--hbv-*` tokens to the kit tokens (`--popover`, `--ink`,
  `--tint-neutral-fill` and so on).
- The CSS setup follows the `ev_lib` README: `build.rs` writes `ev_lib_classes::CLASS_INVENTORY` to
  `uikit-classes.txt` and `TOKENS_CSS` to `assets/tokens.css`. `tailwind.css` uses `@source` for the first
  file and `@import` for the second. Git ignores both files, so they always match `Cargo.lock`.
- `dx` 0.7 runs Tailwind before `cargo`, so on a fresh clone the kit files do not exist yet when Tailwind
  starts. The recipes run `cargo check` (or clippy) first. `build.rs` also writes an empty
  `assets/tailwind.css` when there is none, because `asset!` needs the file to exist. `dx` then replaces it.
- The light switch recolours the kit and the widget at the same time, with no re-render.
