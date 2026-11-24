// Static server for models/hosted/ with CORS. Run: just serve-models [port]
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../models/hosted/', import.meta.url));
const port = Number(process.argv[2] ?? 5299);

// .wasm must be application/wasm, or WebAssembly.instantiateStreaming rejects it; .mjs must be a JavaScript type to load as a module.
const TYPES = {
  '.wasm': 'application/wasm',
  '.mjs': 'text/javascript',
  '.js': 'text/javascript',
  '.json': 'application/json',
  '.onnx': 'application/octet-stream',
  '.bin': 'application/octet-stream',
};

const server = http.createServer((req, res) => {
  // CORP lets a cross-origin-isolated page (COEP `require-corp`, needed for multithreaded wasm) load these files.
  const headers = {
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET, HEAD, OPTIONS',
    'Cross-Origin-Resource-Policy': 'cross-origin',
  };
  if (req.method === 'OPTIONS') {
    res.writeHead(204, headers).end();
    return;
  }
  if (req.method !== 'GET' && req.method !== 'HEAD') {
    res.writeHead(405, headers).end();
    return;
  }
  const requested = decodeURIComponent(new URL(req.url ?? '/', 'http://localhost').pathname);
  const file = path.join(root, requested);
  // A path that escapes the root (`..`) is a 404, not a read of another file.
  if (!file.startsWith(root) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
    res.writeHead(404, headers).end('Not found');
    return;
  }
  const { size } = fs.statSync(file);
  res.writeHead(200, {
    ...headers,
    'Content-Type': TYPES[path.extname(file)] ?? 'application/octet-stream',
    'Content-Length': size,
  });
  if (req.method === 'HEAD') res.end();
  else fs.createReadStream(file).pipe(res);
});

server.listen(port, () => console.log(`Serving ${root} at http://localhost:${port}/`));
