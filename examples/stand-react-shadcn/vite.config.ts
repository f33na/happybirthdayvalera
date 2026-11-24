import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';
import { defineConfig } from 'vite';

// Cross-origin isolation, so the Whisper worker runs onnxruntime on several threads (SharedArrayBuffer). Without
// these headers it falls back to one thread, about 2.5 times slower. Every cross-origin file the page loads must
// then allow it with CORS or `Cross-Origin-Resource-Policy`; the Hub, jsDelivr and `just serve-models` do.
const isolation = { 'Cross-Origin-Opener-Policy': 'same-origin', 'Cross-Origin-Embedder-Policy': 'require-corp' };

// A stock shadcn/ui Vite setup: the `@` alias and the Tailwind v4 plugin. Nothing points into the package
// source; `happybirthdayvalera` resolves through its `exports` map, to the built `dist`.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { '@': path.resolve(import.meta.dirname, './src') } },
  worker: { format: 'es' },
  server: { headers: isolation },
  preview: { headers: isolation },
});
