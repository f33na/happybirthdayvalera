// Real recognition over fixtures/: transformers.js on Node, the same model id, dtype and options as the browser worker.
// The first run downloads the model into packages/react/.cache (gitignored). Needs network once.
import { env, pipeline } from '@huggingface/transformers';
import fs from 'node:fs';
import path from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import { behavior, type Lang } from '../src/core/data';
import { PIPELINE_DTYPE, cleanTranscript, transcribeOptions } from '../src/engines/whisper/pipeline-config';

const root = path.resolve(import.meta.dirname, '../../..');
const MODEL = process.env.HBV_ACCURACY_MODEL === 'tiny' ? 'tiny' : 'base';

interface Expected {
  text: string;
  keywords: string[];
}
const expected = JSON.parse(fs.readFileSync(path.join(root, 'fixtures/expected.json'), 'utf8')) as Record<
  Lang | '$comment',
  Expected | string
>;

/** Reads 16-bit PCM from a WAV file chunk by chunk; the file may carry extra chunks such as FLLR. */
function readWav(file: string): Float32Array {
  const buf = fs.readFileSync(file);
  if (buf.toString('ascii', 0, 4) !== 'RIFF' || buf.toString('ascii', 8, 12) !== 'WAVE') {
    throw new Error(`${file} is not a WAV file`);
  }
  let offset = 12;
  let format: { channels: number; rate: number; bits: number } | null = null;
  while (offset + 8 <= buf.length) {
    const id = buf.toString('ascii', offset, offset + 4);
    const size = buf.readUInt32LE(offset + 4);
    const body = offset + 8;
    if (id === 'fmt ') {
      format = { channels: buf.readUInt16LE(body + 2), rate: buf.readUInt32LE(body + 4), bits: buf.readUInt16LE(body + 14) };
    } else if (id === 'data') {
      if (!format || format.bits !== 16 || format.channels !== 1 || format.rate !== behavior.sampleRate) {
        throw new Error(`${file}: expected 16 kHz mono 16-bit PCM, got ${JSON.stringify(format)}`);
      }
      const end = Math.min(body + size, buf.length);
      const samples = new Float32Array(Math.floor((end - body) / 2));
      for (let i = 0; i < samples.length; i++) samples[i] = buf.readInt16LE(body + i * 2) / 32768;
      return samples;
    }
    offset = body + size + (size % 2); // chunks are word-aligned
  }
  throw new Error(`${file} has no data chunk`);
}

const normalize = (text: string): string => text.toLowerCase().replace(/ё/g, 'е');

type Transcriber = (audio: Float32Array, options: ReturnType<typeof transcribeOptions>) => Promise<{ text: string }>;
let transcribe: Transcriber;

beforeAll(async () => {
  env.cacheDir = path.resolve(import.meta.dirname, '../.cache/models');
  const pipe = await pipeline('automatic-speech-recognition', behavior.models.web[MODEL], {
    dtype: PIPELINE_DTYPE,
    device: 'cpu',
  });
  // The pipeline type is a large union; only its call signature is used.
  transcribe = pipe as unknown as Transcriber;
});

describe(`whisper-${MODEL} accuracy over fixtures/`, () => {
  it.each(['en', 'ru', 'de', 'fr'] as const)('%s: finds every keyword', async (lang) => {
    const entry = expected[lang] as Expected;
    const pcm = readWav(path.join(root, 'fixtures', `${lang}.wav`));
    const { text } = await transcribe(pcm, transcribeOptions(lang));
    const heard = cleanTranscript(text);
    console.log(`[${MODEL}] ${lang}: ${heard}`);
    for (const keyword of entry.keywords) {
      expect(normalize(heard), `"${heard}" should contain "${keyword}"`).toContain(keyword);
    }
  });
});
