export { MicButton, type MicButtonProps } from './react/MicButton';
export { ListenerPill, type ListenerPillProps } from './react/ListenerPill';
export { Waveform, type WaveformProps } from './react/Waveform';
export { useListener, type EngineChoice, type Listener, type UseListenerOptions } from './react/useListener';

export type { Lang, ModelSize } from './core/data';
export type { SpeechEngine } from './core/engine';
export { EngineError } from './core/engine';
export type { ErrorCode, ListenerState } from './core/machine';
export { resolvePreferredLang } from './core/lang';

export { whisperEngine, type WhisperEngineOptions } from './engines/whisper/engine';
export { webSpeechEngine, isWebSpeechSupported } from './engines/webspeech/engine';
