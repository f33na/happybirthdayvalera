import { birthdayPhrases, LANGS } from './data';

/** Lowercase, ё→е, non-letter/digit/space → space, collapse whitespace. */
export function normalizePhrase(text: string): string {
  return text
    .toLowerCase()
    .replace(/ё/g, 'е')
    .replace(/[^\p{L}\p{N}\s]/gu, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

// Phrases are normalized once with the same function, so "с днём" written in the data still matches.
const normalizedPhrases: string[] = LANGS.flatMap((lang) => birthdayPhrases[lang].map(normalizePhrase));

/** True when the text contains a birthday phrase of any language. */
export function isBirthdayPhrase(text: string): boolean {
  const normalized = normalizePhrase(text);
  return normalizedPhrases.some((phrase) => normalized.includes(phrase));
}
