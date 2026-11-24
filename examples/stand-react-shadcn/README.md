# stand-react-shadcn

A "voice dictation composer" built the way a real shadcn/ui app is built, with `happybirthdayvalera` added the
way an outside app would add it.

## Run

From the repository root:

```sh
just stand-react-shadcn        # builds the package, then starts the stand on http://localhost:5201
```

Open `http://localhost:5201/?mode=dark` to start in dark mode. The sun and moon button toggles `.dark` on `<html>`.
Use Chrome or Edge for the microphone.
The first dictation downloads the Whisper model: `tiny` (about 40 MB) from the Hub by default. With
`HBV_MODEL_BASE_URL` set, the stand uses `base` (about 80 MB) from your server; `HBV_MODEL=tiny|base|small` picks
another size.

## What it proves

- **Consumer setup.** The stand imports `happybirthdayvalera` and `happybirthdayvalera/style.css` only through the
  package `exports`, which resolve to the built `dist`. There is no `src/` path and no alias into the package.
- **shadcn app.** `components.json`, `@/lib/utils` and `components/ui/*` (`button`, `card`, `textarea`, `badge`,
  `tooltip`, `tabs`) were added with the pinned `shadcn` CLI (preset `vega`, base colour `neutral`).
- **Registry item.** `src/components/voice-listener.tsx` was installed with the real CLI from the local build:
  `npx shadcn add <repo>/registry/public/r/voice-listener.json`. The first tab shows it.
- **`MicButton asChild`.** The second tab uses our `MicButton` over the app's own
  `<Button variant="outline" size="icon">`, with `theme: 'shadcn'` and `ListenerPill` mounted on `pillVisible`.
- **Live dark mode.** The widget tokens resolve on the widget element, so toggling `.dark` recolours both paths
  with no re-render of the widget.
- **Dictation.** Both tabs bind the `Textarea` with `useDictation` (the registry item through its `target` prop):
  drafts appear at the caret while you speak, each pause commits a segment, and Cmd/Ctrl+Z removes the last one.
  The microphone stays on until a second tap, Esc or 8 s without speech. The badge shows the language.

The Vite production build also proves that the Whisper worker shipped in the package is picked up by Vite:
`vite build` emits it as `assets/whisper-worker-*.js`.
