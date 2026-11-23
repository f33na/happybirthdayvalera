// A minimal stand-in for the shadcn `button` component, used only to type-check the registry item.
import * as React from 'react';

export interface ButtonProps extends React.ComponentProps<'button'> {
  variant?: 'default' | 'destructive' | 'outline' | 'secondary' | 'ghost' | 'link';
  size?: 'default' | 'sm' | 'lg' | 'icon';
  asChild?: boolean;
}

export function Button({ variant: _variant, size: _size, asChild: _asChild, ...props }: ButtonProps) {
  return <button {...props} />;
}
