import { describe, expect, it } from 'vitest';
import { isBirthdayPhrase, normalizePhrase } from '../core/easter';
import { readRootJson } from './helpers';

const expected = readRootJson<Record<string, { text: string }>>('fixtures/expected.json');

describe('easter egg', () => {
  it('normalizes: case, ё, punctuation, whitespace', () => {
    expect(normalizePhrase('  С днём,   рождения!  ')).toBe('с днем рождения');
    expect(normalizePhrase("Joyeux anniversaire, l'ami")).toBe('joyeux anniversaire l ami');
  });

  it.each(['en', 'ru', 'de', 'fr'])('the %s fixture phrase triggers it', (lang) => {
    const entry = expected[lang];
    expect(entry).toBeDefined();
    expect(isBirthdayPhrase(entry?.text ?? '')).toBe(true);
  });

  it('matches a phrase of another language than the selected one', () => {
    expect(isBirthdayPhrase('Alles Gute zum Geburtstag')).toBe(true);
    expect(isBirthdayPhrase('HERZLICHEN GLÜCKWUNSCH ZUM GEBURTSTAG!')).toBe(true);
  });

  it('does not fire on ordinary text', () => {
    expect(isBirthdayPhrase('The weather is lovely today.')).toBe(false);
    expect(isBirthdayPhrase('')).toBe(false);
  });
});
