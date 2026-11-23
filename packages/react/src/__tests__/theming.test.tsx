import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { createRef } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { EvListenerPill, EvMicButton } from '../evinvest';
import { ListenerPill, type LangButtonProps } from '../react/ListenerPill';
import { MicButton, micButtonProps } from '../react/MicButton';
import type { Listener } from '../react/useListener';
import { readRootJson } from './helpers';
import fs from 'node:fs';
import path from 'node:path';

function fakeListener(patch: Partial<Listener> = {}): Listener {
  return {
    state: 'idle',
    error: null,
    progress: 0,
    text: '',
    interim: '',
    lang: 'en',
    setLang: vi.fn(),
    cycleLang: vi.fn(),
    start: vi.fn(),
    stop: vi.fn(),
    toggle: vi.fn(),
    reset: vi.fn(),
    celebrating: false,
    levelRef: { current: 0 },
    nothingHeard: false,
    langLocked: false,
    theme: 'default',
    ...patch,
  };
}

afterEach(cleanup);

describe('data-hbv-theme', () => {
  it('renders the listener theme on both roots', () => {
    const listener = fakeListener({ theme: 'shadcn' });
    const { container } = render(
      <>
        <MicButton listener={listener} />
        <ListenerPill listener={listener} />
      </>,
    );
    expect(container.querySelector('.hbv-mic')?.getAttribute('data-hbv-theme')).toBe('shadcn');
    expect(container.querySelector('.hbv-pill')?.getAttribute('data-hbv-theme')).toBe('shadcn');
  });

  it('lets a component override the theme', () => {
    const { container } = render(<MicButton listener={fakeListener({ theme: 'shadcn' })} theme="evinvest" />);
    expect(container.querySelector('.hbv-mic')?.getAttribute('data-hbv-theme')).toBe('evinvest');
  });

  it('defaults to "default"', () => {
    const { container } = render(<ListenerPill listener={fakeListener()} />);
    expect(container.querySelector('.hbv-pill')?.getAttribute('data-hbv-theme')).toBe('default');
  });
});

describe('class slots', () => {
  it('appends host classes after the contract classes', () => {
    const { container } = render(
      <>
        <MicButton
          listener={fakeListener()}
          className="a"
          classNames={{ root: 'b', ring: 'ring-x', icon: 'icon-x' }}
        />
        <ListenerPill
          listener={fakeListener()}
          className="p"
          classNames={{ root: 'q', wave: 'w', strand: 's', status: 'st', text: 'tx', dots: 'd', lang: 'l' }}
        />
      </>,
    );
    expect(container.querySelector('.hbv-mic')?.className).toBe('hbv-mic a b');
    expect(container.querySelector('.hbv-mic__ring')?.className).toBe('hbv-mic__ring ring-x');
    expect(container.querySelector('.hbv-mic__icon')?.getAttribute('class')).toBe('hbv-mic__icon icon-x');
    expect(container.querySelector('.hbv-pill')?.className).toBe('hbv-pill p q');
    expect(container.querySelector('.hbv-wave')?.getAttribute('class')).toBe('hbv-wave w');
    expect(container.querySelector('.hbv-wave__strand')?.getAttribute('class')).toBe('hbv-wave__strand s');
    expect(container.querySelector('.hbv-pill__status')?.className).toBe('hbv-pill__status st');
    expect(container.querySelector('.hbv-dots')?.className).toBe('hbv-dots d');
    expect(container.querySelector('.hbv-lang')?.className).toBe('hbv-lang l');
  });

  it('adds the text slot class', () => {
    const { container } = render(
      <ListenerPill listener={fakeListener({ text: 'hi' })} classNames={{ text: 'tx' }} />,
    );
    expect(container.querySelector('.hbv-pill__text')?.className).toBe('hbv-pill__text tx');
  });
});

describe('MicButton asChild', () => {
  it('renders the child with merged class, aria and the ring and icon', () => {
    const listener = fakeListener({ state: 'listening' });
    render(
      <MicButton listener={listener} asChild className="host" classNames={{ root: 'slot' }}>
        <button className="child" data-x="1">
          <span data-testid="own">own child</span>
        </button>
      </MicButton>,
    );
    const button = screen.getByRole('button');
    expect(button.className).toBe('hbv-mic host slot child');
    expect(button.getAttribute('aria-pressed')).toBe('true');
    expect(button.getAttribute('aria-label')).toBe('Stop listening');
    expect(button.getAttribute('data-state')).toBe('listening');
    expect(button.getAttribute('data-hbv-theme')).toBe('default');
    expect(button.getAttribute('data-x')).toBe('1');
    expect(button.querySelector('.hbv-mic__ring')).not.toBeNull();
    expect(button.querySelector('.hbv-mic__icon')).not.toBeNull();
    expect(screen.getByTestId('own')).toBeTruthy();
    expect(document.querySelectorAll('button')).toHaveLength(1);
  });

  it('composes onClick: the child first, then toggle, unless the child prevents default', () => {
    const order: string[] = [];
    const listener = fakeListener({ toggle: vi.fn(() => void order.push('toggle')) });
    const { rerender } = render(
      <MicButton listener={listener} asChild>
        <button onClick={() => order.push('child')} />
      </MicButton>,
    );
    fireEvent.click(screen.getByRole('button'));
    expect(order).toEqual(['child', 'toggle']);

    order.length = 0;
    rerender(
      <MicButton listener={listener} asChild>
        <button onClick={(e) => (e.preventDefault(), order.push('child'))} />
      </MicButton>,
    );
    fireEvent.click(screen.getByRole('button'));
    expect(order).toEqual(['child']);
  });

  it('forwards the ref to the child element', () => {
    const ref = createRef<HTMLButtonElement>();
    const childRef = createRef<HTMLButtonElement>();
    render(
      <MicButton listener={fakeListener()} asChild ref={ref}>
        <button ref={childRef} />
      </MicButton>,
    );
    const button = screen.getByRole('button');
    expect(ref.current).toBe(button);
    expect(childRef.current).toBe(button);
  });

  it('throws a clear error without a single element child', () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    expect(() => render(<MicButton listener={fakeListener()} asChild />)).toThrow(/exactly one React element/);
  });
});

describe('micButtonProps', () => {
  it('returns the aria, data and click props', () => {
    const listener = fakeListener({ state: 'requesting', lang: 'fr' });
    const props = micButtonProps(listener);
    expect(props).toMatchObject({
      type: 'button',
      'aria-pressed': true,
      'aria-label': 'Arrêter l’écoute',
      'data-state': 'requesting',
      'data-hbv-theme': 'default',
    });
    props.onClick({} as never);
    expect(listener.toggle).toHaveBeenCalledOnce();
  });
});

describe('ListenerPill components', () => {
  it('renders a custom LangButton with the button props', () => {
    const listener = fakeListener();
    function LangButton({ children, ...props }: LangButtonProps) {
      return (
        <button {...props} data-custom="yes">
          {children}
        </button>
      );
    }
    render(<ListenerPill listener={listener} components={{ LangButton }} classNames={{ lang: 'chip' }} />);
    const chip = screen.getByRole('button');
    expect(chip.getAttribute('data-custom')).toBe('yes');
    expect(chip.className).toBe('hbv-lang chip');
    expect(chip.getAttribute('type')).toBe('button');
    expect(chip.getAttribute('aria-label')).toBe('Language: English. Switch language');
    fireEvent.click(chip);
    expect(listener.cycleLang).toHaveBeenCalledOnce();
  });
});

describe('evinvest adapter', () => {
  it('renders a native button with aria-pressed and aria-label, and the kit classes after hbv-mic', () => {
    const listener = fakeListener({ state: 'listening' });
    render(<EvMicButton listener={listener} />);
    const button = screen.getByRole('button');
    expect(button.tagName).toBe('BUTTON');
    expect(button.getAttribute('aria-pressed')).toBe('true');
    expect(button.getAttribute('aria-label')).toBe('Stop listening');
    expect(button.getAttribute('data-hbv-theme')).toBe('evinvest');
    expect(button.classList.contains('hbv-mic')).toBe(true);
    expect(button.classList.contains('shrink-0')).toBe(true); // from the kit's buttonVariants
    fireEvent.click(button);
    expect(listener.toggle).toHaveBeenCalledOnce();
  });

  it('renders the pill with the kit Badge over a native language button', () => {
    const listener = fakeListener();
    const { container } = render(<EvListenerPill listener={listener} />);
    expect(container.querySelector('.hbv-pill')?.getAttribute('data-hbv-theme')).toBe('evinvest');
    const chip = screen.getByRole('button');
    expect(chip.tagName).toBe('BUTTON');
    expect(chip.classList.contains('hbv-lang')).toBe(true);
    expect(chip.className.split(' ').length).toBeGreaterThan(1); // the Badge classes were merged in
    expect(chip.getAttribute('aria-label')).toBe('Language: English. Switch language');
    expect(chip.getAttribute('type')).toBe('button');
    fireEvent.click(chip);
    expect(listener.cycleLang).toHaveBeenCalledOnce();
  });
});

describe('generated stylesheet', () => {
  const css = fs.readFileSync(path.resolve(process.cwd(), 'src/styles.css'), 'utf8');

  it('declares tokens on the widget roots with zero specificity, not on :root', () => {
    expect(css).toContain(':where(.hbv-mic, .hbv-pill) {\n  --hbv-color-pill-bg');
    expect(css).not.toMatch(/^:root/m);
  });

  it('contains both presets, every selector inside :where()', () => {
    expect(css).toContain('[data-hbv-theme="shadcn"]');
    expect(css).toContain('[data-hbv-theme="evinvest"]');
    expect(css).toContain('--hbv-color-pill-bg: var(--popover);');
    expect(css).toContain('--hbv-color-text: var(--ink);');
    const selectors = [...css.matchAll(/^([^\s/*@}][^{]*)\{/gm)].map((m) => m[1] ?? '');
    for (const sel of selectors.filter((s) => s.includes('data-hbv-theme'))) {
      expect(sel.trimStart().startsWith(':where(')).toBe(true);
    }
  });

  it('keeps the crate asset byte-identical', () => {
    const asset = fs.readFileSync(
      path.resolve(process.cwd(), '../../crates/happybirthdayvalera/assets/listener.css'),
      'utf8',
    );
    expect(asset).toBe(css);
  });

  it('has the active icon token in the contract tokens', () => {
    const tokens = readRootJson<{ color: Record<string, string> }>('shared/design-tokens.json');
    expect(tokens.color['mic-icon-active']).toBe('#ffffff');
    expect(css).toContain('--hbv-color-mic-icon-active: #ffffff;');
  });
});
