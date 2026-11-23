# stand-react-evinvest

A "voice dictation composer" built only from `@evinvest/uikit` 0.36.1, with the widget from
`happybirthdayvalera/evinvest`.

## Run

From the repository root:

```sh
just stand-react-evinvest      # builds the package, then starts the stand on http://localhost:5202
```

Open `http://localhost:5202/?mode=light` to start in light mode. The sun button toggles the kit's own `.light`
class on a wrapper. Dark is the kit default. Use Chrome or Edge for the microphone. The first dictation
downloads the Whisper `base` model (about 80 MB).

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
- **Composer.** Card, Textarea, Button, Badge and Tooltip come from the kit. The final text of each utterance is
  appended to the Textarea, and the pill is mounted on `pillVisible`.
