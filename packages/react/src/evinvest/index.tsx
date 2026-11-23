// Adapter for @evinvest/uikit. It is a separate entry (`happybirthdayvalera/evinvest`) and the main entry never
// imports it, so apps without the kit do not need the optional peer.
// Rule: no Tailwind classes of our own here. Tailwind in the host cannot scan this package, so every class
// comes from the kit's own builders (`buttonVariants`, `Badge`) or from the `hbv-*` stylesheet.
import { Badge, buttonVariants, cn, type ButtonSize, type ButtonVariant } from '@evinvest/uikit';
import { forwardRef } from 'react';
import { ListenerPill, type LangButtonProps, type ListenerPillProps } from '../react/ListenerPill';
import { MicButton, micButtonProps, type MicButtonProps } from '../react/MicButton';

export interface EvMicButtonProps extends Omit<MicButtonProps, 'asChild' | 'children' | 'theme'> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

/**
 * The mic as a native `<button>` that wears the kit's button classes. The kit's `Button` is not used: it
 * does not pass `aria-pressed` and `aria-label` through reliably, and the widget needs both.
 */
export const EvMicButton = forwardRef<HTMLButtonElement, EvMicButtonProps>(function EvMicButton(
  { variant = 'secondary', size = 'md', className, ...rest },
  ref,
) {
  // The widget stylesheet sits in `@layer components`, so the kit's classes win on every property they set,
  // including the background. The active state therefore has to come from the kit too: the primary variant.
  const pressed = micButtonProps(rest.listener, 'evinvest')['aria-pressed'];
  return (
    <MicButton
      ref={ref}
      {...rest}
      theme="evinvest"
      className={cn(buttonVariants({ variant: pressed ? 'primary' : variant, size, icon: true }), className)}
    />
  );
});

/** The language chip as the kit's `Badge` rendered over a real `<button>`. */
function EvLangButton({ children, ...props }: LangButtonProps) {
  return (
    <Badge asChild tone="neutral">
      <button {...props}>{children}</button>
    </Badge>
  );
}

export type EvListenerPillProps = Omit<ListenerPillProps, 'theme' | 'components'>;

export function EvListenerPill(props: EvListenerPillProps) {
  return <ListenerPill {...props} theme="evinvest" components={{ LangButton: EvLangButton }} />;
}
