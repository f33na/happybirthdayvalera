import { defineConfig, type Plugin } from 'vite';

const PLACEHOLDER = '__HBV_WORKER_FILE__';

/**
 * In source the worker is `./worker.ts`, so Vite dev servers and tests resolve it. The published file name is
 * `./whisper-worker.js`, a sibling of `index.js`. A placeholder keeps Vite's own worker/asset plugins from
 * bundling the worker into the library, and the literal `new URL("./whisper-worker.js", import.meta.url)` is
 * restored at the end so that the consumer's bundler (Vite, webpack 5, Next) sees the pattern it knows.
 */
function workerUrlPlugin(): Plugin {
  return {
    name: 'hbv-worker-url',
    apply: 'build',
    enforce: 'pre',
    transform(code, id) {
      if (!id.endsWith('engines/whisper/engine.ts')) return null;
      const source = "new URL('./worker.ts', import.meta.url)";
      if (!code.includes(source)) throw new Error('hbv-worker-url: the worker URL expression was not found');
      return code.replace(source, `new URL(${PLACEHOLDER}, import.meta.url)`);
    },
    renderChunk(code) {
      return code.includes(PLACEHOLDER) ? code.replaceAll(PLACEHOLDER, '"./whisper-worker.js"') : null;
    },
  };
}

export default defineConfig({
  plugins: [workerUrlPlugin()],
  build: {
    target: 'es2022',
    sourcemap: false,
    minify: false,
    emptyOutDir: true,
    lib: {
      // The kit adapter is its own entry, so the main entry never pulls in the optional peer.
      entry: { index: 'src/index.ts', evinvest: 'src/evinvest/index.tsx' },
      formats: ['es', 'cjs'],
      fileName: (format, entry) => `${entry}.${format === 'es' ? 'js' : 'cjs'}`,
    },
    rollupOptions: {
      external: ['react', 'react/jsx-runtime', 'react-dom', '@evinvest/uikit'],
      // Next.js App Router: the hooks and components below must run on the client.
      output: { banner: '"use client";' },
    },
  },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.{ts,tsx}'],
  },
});
