// The only module that reads the shared JSON contract files. Everything else imports typed values from here.
import behaviorJson from '../../../../shared/behavior.json';
import localesJson from '../../../../shared/locales.json';
import machineJson from '../../../../shared/machine.json';
import tokensJson from '../../../../shared/design-tokens.json';

export type Lang = 'en' | 'ru' | 'de' | 'fr';
export type ModelSize = 'tiny' | 'base' | 'small';

export type StringKey =
  | 'status.idle'
  | 'status.requesting'
  | 'status.loadingModel'
  | 'status.listening'
  | 'status.transcribing'
  | 'status.nothingHeard'
  | 'error.micDenied'
  | 'error.micUnavailable'
  | 'error.unsupported'
  | 'error.modelFailed'
  | 'error.transcribeFailed'
  | 'action.start'
  | 'action.stop'
  | 'action.retry'
  | 'action.switchLang'
  | 'label.widget';

export interface LanguageInfo {
  code: Lang;
  short: string;
  name: string;
}

export interface WaveStrand {
  freq: number;
  speed: number;
  phase: number;
  amp: number;
  opacity: number;
}

export interface WaveTokens {
  viewBoxWidth: number;
  viewBoxHeight: number;
  points: number;
  strokeWidth: number;
  strokePad: number;
  idleAmp: number;
  restAmp: number;
  reducedMotionAmp: number;
  levelGain: number;
  attack: number;
  release: number;
  strands: WaveStrand[];
}

export interface Behavior {
  sampleRate: number;
  vadThresholdRms: number;
  minSpeechMs: number;
  silenceMs: number;
  maxDurationMs: number;
  interimEveryMs: number;
  easterEggMs: number;
  langCycle: Lang[];
  persistLangKey: string;
  models: { default: ModelSize; web: Record<ModelSize, string> };
}

// Assigning the JSON to these explicit types makes `tsc` fail when the contract files drift from them.
export const LANGS: readonly Lang[] = localesJson.languages.map((l) => l.code as Lang);
export const languages: readonly LanguageInfo[] = localesJson.languages as LanguageInfo[];
export const strings: Record<Lang, Record<StringKey, string>> = localesJson.strings;
const phrases = localesJson.birthdayPhrases;
export const birthdayPhrases: Record<Lang, string[]> = { en: phrases.en, ru: phrases.ru, de: phrases.de, fr: phrases.fr };
export const wave: WaveTokens = tokensJson.wave;
// Widening cast: JSON gives string[] for langCycle; a unit test checks every entry is a supported Lang.
export const behavior = behaviorJson as Behavior;
export const machineSpec = machineJson;
