import { afterEach, describe, expect, it } from 'vitest';
import { behavior } from '../core/data';
import { detectLang, nextLang, persistLang, readStoredLang, resolvePreferredLang } from '../core/lang';

describe('resolvePreferredLang', () => {
  it.each([
    [['ru-RU'], 'ru'],
    [['pt-BR', 'de'], 'de'],
    [['pt-BR'], 'en'],
    [[], 'en'],
    [['FR_ca'], 'fr'],
    [['en-GB'], 'en'],
    [['DE-at', 'fr'], 'de'],
    [['xx', '', 'ru'], 'ru'],
  ] as const)('%j -> %s', (list, expected) => {
    expect(resolvePreferredLang(list)).toBe(expected);
  });
});

describe('storage', () => {
  afterEach(() => localStorage.clear());

  it('persists under the key from behavior.json', () => {
    persistLang('de');
    expect(localStorage.getItem(behavior.persistLangKey)).toBe('de');
    expect(readStoredLang()).toBe('de');
  });

  it('ignores a stored value that is not a supported language', () => {
    localStorage.setItem(behavior.persistLangKey, 'klingon');
    expect(readStoredLang()).toBeNull();
  });

  it('prefers storage over the browser only when asked to', () => {
    persistLang('fr');
    expect(detectLang(true)).toBe('fr');
    expect(detectLang(false)).toBe('en'); // jsdom reports en-US
  });
});

describe('nextLang', () => {
  it('follows langCycle and wraps', () => {
    expect(nextLang('en')).toBe('ru');
    expect(nextLang('ru')).toBe('de');
    expect(nextLang('de')).toBe('fr');
    expect(nextLang('fr')).toBe('en');
  });
});
