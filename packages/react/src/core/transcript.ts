import { behavior } from './data';
import { normalizePhrase } from './easter';

const hallucinations: ReadonlySet<string> = new Set(behavior.hallucinations.map(normalizePhrase));

/**
 * The speech in an engine result, or '' when there is none (CONTRACT.md, "Non-speech output"). Whisper writes
 * sound tags such as `[BLANK_AUDIO]`, `(смеется)` or `*applause*` for audio without speech, and on silence it
 * can invent a stock subtitle phrase. A listed phrase counts only when it is the whole text, so real speech that
 * contains "thank you" stays.
 */
export function cleanTranscript(text: string): string {
  const speech = text
    .replace(/\[[^\]]*\]|\([^)]*\)|\*[^*]*\*|[♪♫]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
  if (!/[\p{L}\p{N}]/u.test(speech)) return '';
  return hallucinations.has(normalizePhrase(speech)) ? '' : speech;
}
