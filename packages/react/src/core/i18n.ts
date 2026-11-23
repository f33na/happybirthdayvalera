import { languages, strings, type Lang, type StringKey } from './data';
import type { ErrorCode } from './machine';

export function t(lang: Lang, key: StringKey, vars: Record<string, string | number> = {}): string {
  const template = strings[lang][key];
  return template.replace(/\{(\w+)\}/g, (match, name: string) => {
    const value = vars[name];
    return value === undefined ? match : String(value);
  });
}

export function langName(lang: Lang): string {
  return languages.find((l) => l.code === lang)?.name ?? lang;
}

export function langShort(lang: Lang): string {
  return languages.find((l) => l.code === lang)?.short ?? lang.toUpperCase();
}

const ERROR_KEYS: Record<ErrorCode, StringKey> = {
  'mic-denied': 'error.micDenied',
  'mic-unavailable': 'error.micUnavailable',
  unsupported: 'error.unsupported',
  'model-failed': 'error.modelFailed',
  'transcribe-failed': 'error.transcribeFailed',
};

export function errorKey(code: ErrorCode): StringKey {
  return ERROR_KEYS[code];
}
