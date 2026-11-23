import { describe, expect, it } from 'vitest';
import { behavior, LANGS, languages, strings, type StringKey } from '../core/data';
import { t } from '../core/i18n';

const placeholders = (s: string): string[] => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1] ?? '').sort();
const enKeys = Object.keys(strings.en).sort() as StringKey[];

describe('locales', () => {
  it('has the four supported languages', () => {
    expect([...LANGS]).toEqual(['en', 'ru', 'de', 'fr']);
    expect(languages.map((l) => l.short)).toEqual(['EN', 'RU', 'DE', 'FR']);
  });

  it('keeps langCycle inside the supported languages', () => {
    expect(behavior.langCycle.every((l) => LANGS.includes(l))).toBe(true);
  });

  it.each(LANGS)('%s has exactly the keys of en, with the same placeholders', (lang) => {
    expect(Object.keys(strings[lang]).sort()).toEqual(enKeys);
    for (const key of enKeys) {
      expect(placeholders(strings[lang][key]), `${lang} ${key}`).toEqual(placeholders(strings.en[key]));
      expect(strings[lang][key].trim(), `${lang} ${key}`).not.toBe('');
    }
  });

  it('fills placeholders', () => {
    expect(t('en', 'status.loadingModel', { percent: 42 })).toBe('Loading model 42%');
    expect(t('fr', 'status.loadingModel', { percent: 7 })).toBe('Chargement du modèle 7 %');
    expect(t('de', 'action.switchLang', { lang: 'Deutsch' })).toBe('Sprache: Deutsch. Sprache wechseln');
  });
});
