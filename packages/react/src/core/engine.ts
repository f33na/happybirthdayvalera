import type { Lang, ModelSize } from './data';

export type { Lang, ModelSize };

export interface SpeechEngine {
  load(model: ModelSize, onProgress: (fraction: number) => void): Promise<void>;
  transcribe(pcm: Float32Array, lang: Lang): Promise<string>;
  dispose?(): void;
  /**
   * Optional, an addition to the shared contract: engines that listen to the microphone themselves
   * (Web Speech) start here, when the controller enters `listening`. Engines that get the PCM buffer
   * do not implement it.
   */
  begin?(lang: Lang): void;
}

/** Why an engine failed, so the controller can map it to an error code. */
export class EngineError extends Error {
  readonly code: 'unsupported' | 'model-failed' | 'transcribe-failed';
  constructor(code: EngineError['code'], message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = 'EngineError';
    this.code = code;
  }
}
