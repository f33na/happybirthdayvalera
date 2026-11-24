// A minimal stand-in for the shadcn `cn` helper, used only to type-check the registry item.
export function cn(...inputs: (string | false | null | undefined)[]): string {
  return inputs.filter(Boolean).join(' ');
}
