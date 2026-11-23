import { behavior, LANGS, type Lang } from './data';

export function isLang(value: unknown): value is Lang {
  return typeof value === 'string' && (LANGS as readonly string[]).includes(value);
}

/** Walks the tags in order and returns the first supported primary subtag, else `en`. */
export function resolvePreferredLang(list: readonly string[]): Lang {
  for (const tag of list) {
    const primary = (tag.split(/[-_]/, 1)[0] ?? '').toLowerCase();
    if (isLang(primary)) return primary;
  }
  return 'en';
}

export function browserLanguages(): string[] {
  if (typeof navigator === 'undefined') return [];
  const list = navigator.languages ?? [];
  if (list.length > 0) return [...list];
  return navigator.language ? [navigator.language] : [];
}

/** Storage can be blocked (private mode, sandboxed iframe), so every access is guarded. */
export function readStoredLang(): Lang | null {
  try {
    const value = localStorage.getItem(behavior.persistLangKey);
    return isLang(value) ? value : null;
  } catch {
    return null;
  }
}

export function persistLang(lang: Lang): void {
  try {
    localStorage.setItem(behavior.persistLangKey, lang);
  } catch {
    // Persisting is a convenience; a blocked storage must not break voice input.
  }
}

/** The language when `lang` is not forced: storage first (only with `persist`), then the browser. */
export function detectLang(persist: boolean): Lang {
  const stored = persist ? readStoredLang() : null;
  return stored ?? resolvePreferredLang(browserLanguages());
}

export function nextLang(current: Lang): Lang {
  const cycle = behavior.langCycle;
  const index = cycle.indexOf(current);
  return cycle[(index + 1) % cycle.length] ?? 'en';
}
