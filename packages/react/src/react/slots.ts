export type Theme = 'default' | 'shadcn' | 'evinvest';

export interface MicClassNames {
  root?: string;
  ring?: string;
  icon?: string;
}

export interface PillClassNames {
  root?: string;
  wave?: string;
  strand?: string;
  status?: string;
  text?: string;
  dots?: string;
  lang?: string;
}

/** Joins class names. The contract class always comes first, host classes after it. */
export function cx(...parts: (string | false | null | undefined)[]): string {
  return parts.filter(Boolean).join(' ');
}
