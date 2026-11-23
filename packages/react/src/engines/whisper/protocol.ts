import type { Lang } from '../../core/data';

/** Main → worker. The `pcm` buffer of `transcribe` is transferred. */
export type WorkerRequest =
  | { type: 'load'; model: string; wasmPaths?: string; modelBaseUrl?: string }
  | { type: 'transcribe'; id: number; pcm: Float32Array; lang: Lang };

/** Worker → main. `error.id` is absent for a load error. */
export type WorkerResponse =
  | { type: 'progress'; fraction?: number }
  | { type: 'ready' }
  | { type: 'result'; id: number; text: string }
  | { type: 'error'; id?: number; message: string };

export function isWorkerResponse(value: unknown): value is WorkerResponse {
  if (typeof value !== 'object' || value === null || !('type' in value)) return false;
  switch (value.type) {
    case 'progress':
      return !('fraction' in value) || typeof value.fraction === 'number';
    case 'ready':
      return true;
    case 'result':
      return 'id' in value && typeof value.id === 'number' && 'text' in value && typeof value.text === 'string';
    case 'error':
      return 'message' in value && typeof value.message === 'string';
    default:
      return false;
  }
}
