import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { wave } from '../core/data';
import { strandPath } from '../core/wave';
import type { ListenerState } from '../core/machine';
import { ListenerPill } from '../react/ListenerPill';
import { MicButton } from '../react/MicButton';
import type { Listener } from '../react/useListener';

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

function mockReducedMotion(matches: boolean): void {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
}

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe('MicButton', () => {
  it.each<[ListenerState, boolean]>([
    ['idle', false],
    ['requesting', true],
    ['loading-model', true],
    ['listening', true],
    ['transcribing', false],
    ['error', false],
  ])('%s -> aria-pressed=%s', (state, pressed) => {
    render(<MicButton listener={fakeListener({ state })} />);
    const button = screen.getByRole('button');
    expect(button.getAttribute('aria-pressed')).toBe(String(pressed));
    expect(button.getAttribute('data-state')).toBe(state);
    expect(button.getAttribute('aria-label')).toBe(pressed ? 'Stop listening' : 'Start voice input');
    expect(button.className).toBe('hbv-mic');
    expect(button.querySelector('.hbv-mic__ring')?.getAttribute('aria-hidden')).toBe('true');
  });

  it('labels the button in the selected language', () => {
    render(<MicButton listener={fakeListener({ lang: 'ru', state: 'listening' })} />);
    expect(screen.getByRole('button').getAttribute('aria-label')).toBe('Остановить запись');
  });

  it('toggles on click', () => {
    const listener = fakeListener();
    render(<MicButton listener={listener} />);
    fireEvent.click(screen.getByRole('button'));
    expect(listener.toggle).toHaveBeenCalledOnce();
  });
});

describe('ListenerPill', () => {
  it('renders the contract markup while listening', () => {
    const { container } = render(<ListenerPill listener={fakeListener({ state: 'listening' })} />);
    const pill = container.querySelector('.hbv-pill');
    expect(pill?.getAttribute('role')).toBe('group');
    expect(pill?.getAttribute('aria-label')).toBe('Voice input');
    expect(pill?.getAttribute('data-state')).toBe('listening');
    expect(pill?.getAttribute('data-celebrate')).toBe('false');
    expect(container.querySelectorAll('.hbv-wave__strand')).toHaveLength(3);
    expect(container.querySelector('.hbv-wave')?.getAttribute('viewBox')).toBe('0 0 72 28');
    const status = container.querySelector('.hbv-pill__status');
    expect(status?.textContent).toBe('Listening');
    expect(status?.getAttribute('aria-live')).toBe('polite');
    expect(container.querySelectorAll('.hbv-dots i')).toHaveLength(3);
    expect(container.querySelector('.hbv-dots')?.getAttribute('aria-hidden')).toBe('true');
    const chip = screen.getByRole('button');
    expect(chip.textContent).toBe('EN');
    expect(chip.getAttribute('aria-label')).toBe('Language: English. Switch language');
  });

  it('shows the download percent', () => {
    render(<ListenerPill listener={fakeListener({ state: 'loading-model', progress: 0.426 })} />);
    expect(screen.getByText('Downloading model 43%')).toBeTruthy();
  });

  it('shows the error string and error state', () => {
    const { container } = render(
      <ListenerPill listener={fakeListener({ state: 'error', error: 'mic-denied', lang: 'de' })} />,
    );
    expect(container.querySelector('.hbv-pill')?.getAttribute('data-state')).toBe('error');
    expect(screen.getByText('Mikrofonzugriff verweigert')).toBeTruthy();
  });

  it('shows "nothing heard" after an empty result', () => {
    render(<ListenerPill listener={fakeListener({ nothingHeard: true })} />);
    expect(screen.getByText('Didn’t catch that')).toBeTruthy();
  });

  it('marks the celebration', () => {
    const { container } = render(<ListenerPill listener={fakeListener({ celebrating: true })} />);
    expect(container.querySelector('.hbv-pill')?.getAttribute('data-celebrate')).toBe('true');
  });

  it('shows interim text while listening and the final text otherwise', () => {
    const { container, rerender } = render(
      <ListenerPill listener={fakeListener({ state: 'listening', interim: 'hap', text: 'old' })} />,
    );
    expect(container.querySelector('.hbv-pill__text')?.textContent).toBe('hap');
    rerender(<ListenerPill listener={fakeListener({ state: 'idle', text: 'done' })} />);
    expect(container.querySelector('.hbv-pill__text')?.textContent).toBe('done');
  });

  it('cycles the language with the chip and hides it when the language is forced', () => {
    const listener = fakeListener();
    const { rerender } = render(<ListenerPill listener={listener} />);
    fireEvent.click(screen.getByRole('button'));
    expect(listener.cycleLang).toHaveBeenCalledOnce();
    rerender(<ListenerPill listener={fakeListener({ langLocked: true })} />);
    expect(screen.queryByRole('button')).toBeNull();
  });

  it('draws a valid path before any frame', () => {
    const { container } = render(<ListenerPill listener={fakeListener()} />);
    const d = container.querySelector('.hbv-wave__strand')?.getAttribute('d');
    expect(d).toBe(strandPath(0, 0, wave.restAmp));
  });
});

describe('Waveform and reduced motion', () => {
  it('draws one static frame and starts no animation', () => {
    mockReducedMotion(true);
    const raf = vi.spyOn(window, 'requestAnimationFrame');
    const { container } = render(<ListenerPill listener={fakeListener({ state: 'listening' })} />);
    const strands = [...container.querySelectorAll('.hbv-wave__strand')];
    strands.forEach((el, i) => {
      expect(el.getAttribute('d')).toBe(strandPath(i, 0, wave.reducedMotionAmp));
    });
    expect(raf).not.toHaveBeenCalled();
  });

  it('animates through requestAnimationFrame otherwise', () => {
    mockReducedMotion(false);
    const raf = vi.spyOn(window, 'requestAnimationFrame').mockImplementation(() => 1);
    render(<ListenerPill listener={fakeListener({ state: 'listening' })} />);
    expect(raf).toHaveBeenCalled();
  });
});
