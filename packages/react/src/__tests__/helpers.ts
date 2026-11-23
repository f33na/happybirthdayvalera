import fs from 'node:fs';
import path from 'node:path';

// `npm run test` runs in packages/react. `import.meta.url` is not a file URL under the jsdom environment.
const root = path.resolve(process.cwd(), '../..');

/** Reads a JSON file from the repository root (shared contract data, fixtures). */
export function readRootJson<T>(relative: string): T {
  // Test data from the repo itself; the assertions that use it check its shape.
  return JSON.parse(fs.readFileSync(path.join(root, relative), 'utf8')) as T;
}
