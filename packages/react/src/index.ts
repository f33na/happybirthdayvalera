export { MicButton, micButtonProps, type MicButtonDomProps, type MicButtonProps } from './react/MicButton';
export { ListenerPill, statusText, type LangButtonProps, type ListenerPillProps } from './react/ListenerPill';
export { Waveform, type WaveformProps } from './react/Waveform';
export { useListener, type EngineChoice, type Listener, type UseListenerOptions } from './react/useListener';
export { useDictation } from './react/useDictation';
export type { TextField } from './core/field';
export { cx, type MicClassNames, type PillClassNames, type Theme } from './react/slots';
export { Slot, type SlotProps } from './react/Slot';

export type { Lang, Mode, ModelSize } from './core/data';
export type { SpeechEngine } from './core/engine';
export { EngineError } from './core/engine';
export type { ErrorCode, ListenerState } from './core/machine';
export { resolvePreferredLang } from './core/lang';
// Headless helpers for hosts that compose their own UI.
export { langName, langShort, t } from './core/i18n';
export { strandPath as wavePath, amplitude as waveAmplitude } from './core/wave';

export { whisperEngine, type WhisperEngineOptions } from './engines/whisper/engine';
export type { Device } from './engines/whisper/pipeline-config';
export { webSpeechEngine, isWebSpeechSupported } from './engines/webspeech/engine';
