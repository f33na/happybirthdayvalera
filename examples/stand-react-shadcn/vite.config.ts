import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';
import { defineConfig } from 'vite';

// A stock shadcn/ui Vite setup: the `@` alias and the Tailwind v4 plugin. Nothing points into the package
// source; `happybirthdayvalera` resolves through its `exports` map, to the built `dist`.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { '@': path.resolve(import.meta.dirname, './src') } },
  worker: { format: 'es' },
});
