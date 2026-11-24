import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// Cross-origin isolation, so the Whisper worker runs onnxruntime on several threads (SharedArrayBuffer). Without
// these headers it falls back to one thread, about 2.5 times slower. Every cross-origin file the page loads must
// then allow it with CORS or `Cross-Origin-Resource-Policy`; the Hub, jsDelivr and `just serve-models` do.
const isolation = { 'Cross-Origin-Opener-Policy': 'same-origin', 'Cross-Origin-Embedder-Policy': 'require-corp' };

// A stock Vite + Tailwind v4 app. Nothing points into the package source; `happybirthdayvalera` and
// `happybirthdayvalera/evinvest` resolve through the `exports` map, to the built `dist`.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  worker: { format: 'es' },
  server: { headers: isolation },
  preview: { headers: isolation },
});
