// Downloads the files for self-hosted models into models/hosted/. Run: just fetch-models [tiny|base|small]
// The default is base: the stands and demos use it whenever HBV_MODEL_BASE_URL is set (see shared/CONTRACT.md).
// Layout (see shared/CONTRACT.md, "Self-hosted models"): <model id>/…, ort/…, ggml-<size>.bin.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = new URL('..', import.meta.url);
const at = (p) => fileURLToPath(new URL(p, root));
const behavior = JSON.parse(fs.readFileSync(at('shared/behavior.json'), 'utf8'));

const size = process.argv[2] ?? 'base';
const modelId = behavior.models.web[size];
if (!modelId) {
  console.error(`Unknown size "${size}". Use one of: ${Object.keys(behavior.models.web).join(', ')}`);
  process.exit(1);
}
const outDir = at('models/hosted');

// What the pipeline reads, from packages/react/src/engines/whisper/pipeline-config.ts: the q8 pair for wasm
// (PIPELINE_DTYPE, EXPECTED_MODEL_FILES) and the fp32/fp16 pair for WebGPU (WEBGPU_DTYPE, WEBGPU_MODEL_FILES).
// `device: 'auto'` uses WebGPU only with self-hosted models, and only for the sizes in AUTO_WEBGPU_MODELS, so only
// those sizes get the WebGPU pair too.
const AUTO_WEBGPU_SIZES = new Set(['base']);
const MODEL_FILES = [
  'config.json',
  'generation_config.json',
  'preprocessor_config.json',
  'tokenizer.json',
  'tokenizer_config.json',
  'onnx/encoder_model_quantized.onnx',
  'onnx/decoder_model_merged_quantized.onnx',
  ...(AUTO_WEBGPU_SIZES.has(size) ? ['onnx/encoder_model.onnx', 'onnx/decoder_model_merged_fp16.onnx'] : []),
];

/** Size on the server, or null when it is not reported. */
async function remoteSize(url) {
  const response = await fetch(url, { method: 'HEAD', redirect: 'follow' });
  if (!response.ok) throw new Error(`HEAD ${url}: ${response.status}`);
  const length = Number(response.headers.get('content-length'));
  return Number.isFinite(length) && length > 0 ? length : null;
}

async function downloadOnce(url, target) {
  const expected = await remoteSize(url);
  if (fs.existsSync(target) && (expected === null || fs.statSync(target).size === expected)) {
    console.log(`skip     ${path.relative(outDir, target)}`);
    return;
  }
  fs.mkdirSync(path.dirname(target), { recursive: true });
  const response = await fetch(url, { redirect: 'follow' });
  if (!response.ok || !response.body) throw new Error(`GET ${url}: ${response.status}`);
  const part = `${target}.part`;
  const out = fs.createWriteStream(part);
  let loaded = 0;
  let shown = 0;
  for await (const chunk of response.body) {
    loaded += chunk.length;
    if (!out.write(chunk)) await new Promise((resolve) => out.once('drain', resolve));
    if (loaded - shown > 8 * 1024 * 1024) {
      shown = loaded;
      process.stdout.write(`\r${path.relative(outDir, target)} ${(loaded / 1e6).toFixed(0)} MB`);
    }
  }
  await new Promise((resolve, reject) => out.end((error) => (error ? reject(error) : resolve())));
  if (expected !== null && loaded !== expected) throw new Error(`${url}: got ${loaded} bytes, expected ${expected}`);
  fs.renameSync(part, target);
  console.log(`\rdone     ${path.relative(outDir, target)} ${(loaded / 1e6).toFixed(1)} MB`);
}

// The network can be slow and flaky; a reset should not throw away the files that are already there.
async function download(url, target) {
  for (let attempt = 1; ; attempt++) {
    try {
      return await downloadOnce(url, target);
    } catch (error) {
      if (attempt === 4) throw error;
      console.warn(`\nretry ${attempt}: ${error.message}`);
    }
  }
}

for (const file of MODEL_FILES) {
  await download(`https://huggingface.co/${modelId}/resolve/main/${file}`, path.join(outDir, modelId, file));
}

// The onnxruntime-web files that the pinned transformers.js version loads.
const ortSource = at('node_modules/@huggingface/transformers/dist');
fs.mkdirSync(path.join(outDir, 'ort'), { recursive: true });
for (const file of fs.readdirSync(ortSource).filter((f) => f.startsWith('ort-wasm'))) {
  const target = path.join(outDir, 'ort', file);
  const source = path.join(ortSource, file);
  if (fs.existsSync(target) && fs.statSync(target).size === fs.statSync(source).size) {
    console.log(`skip     ort/${file}`);
  } else {
    fs.copyFileSync(source, target);
    console.log(`copied   ort/${file}`);
  }
}

// The desktop whisper.cpp file, in the flat layout of behavior.json -> models.desktop.baseUrl.
const ggml = behavior.models.desktop[size];
await download(`${behavior.models.desktop.baseUrl}${ggml}`, path.join(outDir, ggml));
console.log(`Ready: ${outDir}. Serve it with: just serve-models`);
