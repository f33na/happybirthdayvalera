# Stand: Dioxus + shadcn/ui

A small "voice dictation composer" screen. It shows the widget as a host app on shadcn/ui would use it:

- a card with a title and a badge with the active language,
- a textarea bound with `use_dictation`: drafts appear in it while you speak and are replaced by the
  final text of each segment,
- a toolbar row with the mic, the pill while it is visible, and a Send button,
- a dark mode switch.

## Run

```sh
just stand-dioxus-shadcn        # dx serve on http://localhost:8201
just stands-dioxus-build        # dx build of both stands, then clippy
```

You need `dx` 0.7.9 and the `wasm32-unknown-unknown` target. On the first build, `dx` downloads the
Tailwind CLI v4. The real `listener.css` and `whisper-worker.js` come from `just sync-assets`. With the
placeholders, the page renders, but the pill reports a model error.

To load the models from a local host instead of the Hugging Face Hub, run `just fetch-models` and
`just serve-models`, then start the stand with `HBV_MODEL_BASE_URL=http://localhost:5299/`. The stand reads
the variable at build time. With self-hosted models it uses Whisper `base`; set `HBV_MODEL=tiny` (or `small`)
to pick another size.

## What it proves

- The stand uses only the public API of the crate: `use_dictation`, `ListenerConfig`, `ModelSize`, `MicButton`,
  `ListenerPill`, `MicClassNames`, `PillClassNames` and `Theme::Shadcn`.
- `Theme::Shadcn` maps the `--hbv-*` tokens to the shadcn variables (`--popover`, `--secondary`,
  `--chart-1` and so on). The dark mode switch sets `.dark` on a wrapper, so the widget recolours live,
  with no re-render.
- `class` and `class_names` reach the right elements. `listener.css` puts every widget rule in
  `@layer components`, and Tailwind v4 puts utilities in the later `@layer utilities`, so a plain utility
  wins without `!`. The mic size override `[--hbv-size-mic:2.25rem]` shows this. It matches the 36 px
  shadcn icon button.
- Tailwind v4 through the built-in Tailwind step of `dx` 0.7: `dx` finds `tailwind.css` next to
  `Cargo.toml` and writes `assets/tailwind.css` before it compiles the crate. Git ignores that output.

## Attribution

`src/ui.rs` ports the class strings of the shadcn/ui `Button`, `Card`, `Textarea` and `Badge` components,
and `tailwind.css` holds the standard shadcn/ui "neutral" token block. shadcn/ui is MIT licensed,
copyright (c) 2023 shadcn, https://github.com/shadcn-ui/ui. The sun and moon icons are from Lucide
(ISC licence), https://lucide.dev.
