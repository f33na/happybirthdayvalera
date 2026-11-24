# stand-react-evinvest

A "voice dictation composer" built only from `@evinvest/uikit` 0.36.1, with the widget from
`happybirthdayvalera/evinvest`.

## Run

From the repository root:

```sh
just stand-react-evinvest      # builds the package, then starts the stand on http://localhost:5202
```

Open `http://localhost:5202/?mode=light` to start in light mode. The sun button toggles the kit's own `.light`
class on a wrapper. Dark is the kit default. Use Chrome or Edge for the microphone.
The first dictation downloads the Whisper model: `tiny` (about 40 MB) from the Hub by default. With
`HBV_MODEL_BASE_URL` set, the stand uses `base` (about 80 MB) from your server; `HBV_MODEL=tiny|base|small` picks
another size.

## What it proves

- **Kit setup.** `src/index.css` follows the kit README: `@import "tailwindcss"` and
  `@import "@evinvest/uikit/styles/tokens.css"`. Tailwind does not scan `node_modules`, so one `@source` line
  points at the compiled kit.
- **Peers.** The kit loads its shell components with the barrel import, so `@evinvest/mfe` and
  `@evinvest/keymap` are installed, as in any app that uses the kit.
- **Adapter.** `EvMicButton` is a native `<button>` that wears the kit's `buttonVariants` classes and keeps
  `aria-pressed` and `aria-label`. `EvListenerPill` uses the kit's `Badge` over a real `<button>` for the language
  chip. The adapter adds no Tailwind classes of its own.
- **Consumer imports.** Only the public `exports` of the package are used: `happybirthdayvalera`,
  `happybirthdayvalera/evinvest` and `happybirthdayvalera/style.css`.
- **Composer.** Card, Textarea, Button, Badge and Tooltip come from the kit. The Textarea is bound with
  `useDictation`: drafts appear at the caret while you speak, each pause commits a segment, and Cmd/Ctrl+Z removes
  the last one. The pill is mounted on `pillVisible` and does not repeat the draft.
