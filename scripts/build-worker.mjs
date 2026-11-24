// Bundles the Whisper worker (packages/react/src/engines/whisper/worker.ts) with transformers.js into one
// self-contained ESM file. Used by scripts/sync-assets.mjs (Rust asset) and packages/react/scripts/build.mjs
// (npm file), so both ship byte-identical code.
import { build } from 'esbuild';

const entry = new URL('../packages/react/src/engines/whisper/worker.ts', import.meta.url);

/** @returns {Promise<string>} the worker source, without a header. */
export async function buildWorker() {
  const result = await build({
    entryPoints: [entry.pathname],
    bundle: true,
    write: false,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    minify: true,
    legalComments: 'none',
    // Absolute paths and sourcemaps would make the output depend on the machine.
    sourcemap: false,
    logLevel: 'warning',
    // transformers.js ships a ready-made web build under the `default` condition; never pick the Node one.
    conditions: ['browser'],
    mainFields: ['browser', 'module', 'main'],
    define: { 'process.env.NODE_ENV': '"production"' },
  });
  const [file] = result.outputFiles;
  if (!file) throw new Error('esbuild produced no output for the worker');
  return file.text;
}
