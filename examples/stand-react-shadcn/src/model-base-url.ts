import type { ModelSize } from 'happybirthdayvalera';

// Opt-in self-hosted models: `HBV_MODEL_BASE_URL=http://localhost:5299/ just <recipe>` (see the README). Unset: the Hub.
const rawBaseUrl: unknown = import.meta.env.VITE_HBV_MODEL_BASE_URL;
const rawModel: unknown = import.meta.env.VITE_HBV_MODEL;

export const modelBaseUrl: string | undefined = typeof rawBaseUrl === 'string' && rawBaseUrl !== '' ? rawBaseUrl : undefined;

const SIZES: readonly ModelSize[] = ['tiny', 'base', 'small'];

function parseModel(value: unknown): ModelSize | undefined {
  if (value === undefined || value === '') return undefined;
  const size = SIZES.find((s) => s === value);
  // A typo in HBV_MODEL fails at startup instead of silently loading another model.
  if (!size) throw new Error(`HBV_MODEL must be one of ${SIZES.join(', ')}, got "${String(value)}"`);
  return size;
}

/**
 * `HBV_MODEL` wins. A self-hosted server has the bandwidth for a better model, so it gets `base`; without one
 * the library default (`tiny`) applies, because it downloads from the Hub.
 */
export const model: ModelSize | undefined = parseModel(rawModel) ?? (modelBaseUrl ? 'base' : undefined);
