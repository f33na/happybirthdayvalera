// Library build: Vite (ESM + CJS), declaration files, the worker file and the stylesheet.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildWorker } from '../../../scripts/build-worker.mjs';

const pkg = fileURLToPath(new URL('..', import.meta.url));
const resolveBin = (name) => {
  // Workspaces hoist binaries to the repo root.
  for (const dir of [path.join(pkg, 'node_modules/.bin'), path.join(pkg, '../../node_modules/.bin')]) {
    const file = path.join(dir, name);
    if (fs.existsSync(file)) return file;
  }
  throw new Error(`Cannot find ${name}; run npm install`);
};
const exec = (name, args) => execFileSync(resolveBin(name), args, { cwd: pkg, stdio: 'inherit' });

fs.rmSync(path.join(pkg, 'dist'), { recursive: true, force: true });

exec('vite', ['build']);

// Declarations: tsc needs the repo root as rootDir because it reads shared/*.json. Flatten the result.
exec('tsc', ['-p', 'tsconfig.build.json']);
const tmp = path.join(pkg, 'dist/.types-tmp');
fs.renameSync(path.join(tmp, 'packages/react/src'), path.join(pkg, 'dist/types'));
fs.rmSync(tmp, { recursive: true, force: true });

fs.writeFileSync(path.join(pkg, 'dist/whisper-worker.js'), await buildWorker());
fs.copyFileSync(path.join(pkg, 'src/styles.css'), path.join(pkg, 'dist/style.css'));

const size = (file) => (fs.statSync(path.join(pkg, file)).size / 1024).toFixed(1);
console.log(`dist/index.js ${size('dist/index.js')} KiB, whisper-worker.js ${size('dist/whisper-worker.js')} KiB`);
