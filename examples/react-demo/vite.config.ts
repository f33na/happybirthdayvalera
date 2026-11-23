import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// The demo imports the library source, so `npm run dev` needs no library build and picks up edits at once.
// An app that installs the package from npm uses the default import and needs none of these aliases.
const lib = new URL('../../packages/react/src/', import.meta.url).pathname;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: [
      { find: /^@\//, replacement: new URL('./src/', import.meta.url).pathname },
      { find: 'happybirthdayvalera/evinvest', replacement: `${lib}evinvest/index.tsx` },
      { find: 'happybirthdayvalera/style.css', replacement: `${lib}styles.css` },
      { find: /^happybirthdayvalera$/, replacement: `${lib}index.ts` },
    ],
  },
  worker: { format: 'es' },
});
