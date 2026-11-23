import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// A stock Vite + Tailwind v4 app. Nothing points into the package source; `happybirthdayvalera` and
// `happybirthdayvalera/evinvest` resolve through the `exports` map, to the built `dist`.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  worker: { format: 'es' },
});
