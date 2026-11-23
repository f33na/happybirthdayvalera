// Opt-in self-hosted models: `HBV_MODEL_BASE_URL=http://localhost:5299/ just <recipe>` (see the README). Unset: the Hub.
const raw: unknown = import.meta.env.VITE_HBV_MODEL_BASE_URL;

export const modelBaseUrl: string | undefined = typeof raw === 'string' && raw !== '' ? raw : undefined;
