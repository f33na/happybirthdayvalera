import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { useRef, useState } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FieldBinding, formatInsertion } from '../core/field';
import type * as MicModule from '../core/mic';
import type { MicBlock } from '../core/mic';
import { ListenerPill } from '../react/ListenerPill';
import { MicButton } from '../react/MicButton';
import { useDictation } from '../react/useDictation';

const mic = vi.hoisted(() => ({ onBlock: null as null | ((block: MicBlock) => void), stop: () => undefined }));
vi.mock('../core/mic', async (original) => ({
  ...(await original<typeof MicModule>()),
  openMic: vi.fn(async (onBlock: (block: MicBlock) => void) => {
    mic.onBlock = onBlock;
    return { stop: mic.stop };
  }),
}));

/**
 * jsdom has no execCommand. This stand-in does what a browser does for 'insertText': it replaces the
 * selection and fires an input event. The log shows which writes would land on the undo stack.
 */
const undoable: string[] = [];
function installExecCommand(): void {
  Object.defineProperty(document, 'execCommand', {
    configurable: true,
    value: (command: string, _ui: boolean, text: string) => {
      const field = document.activeElement;
      if (command !== 'insertText' || !(field instanceof HTMLTextAreaElement)) return false;
      const { selectionStart: s, selectionEnd: e, value } = field;
      // Not `field.value =`: a browser edit does not go through the JS setter that React tracks.
      Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set?.call(field, value.slice(0, s) + text + value.slice(e));
      field.setSelectionRange(s + text.length, s + text.length);
      field.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText' }));
      undoable.push(text);
      return true;
    },
  });
}

function field(value = '', focus = true) {
  const textarea = document.createElement('textarea');
  textarea.value = value;
  document.body.append(textarea);
  if (focus) textarea.focus();
  const inputs = vi.fn();
  textarea.addEventListener('input', inputs);
  const onEscape = vi.fn(() => true);
  const binding = new FieldBinding();
  const detach = binding.attach(textarea, onEscape);
  return { textarea, binding, inputs, onEscape, detach };
}

/** A keystroke of the user: the value changes and the field fires input. */
function userTypes(textarea: HTMLTextAreaElement, at: number, text: string): void {
  textarea.value = textarea.value.slice(0, at) + text + textarea.value.slice(at);
  textarea.setSelectionRange(at + text.length, at + text.length);
  textarea.dispatchEvent(new Event('input', { bubbles: true }));
}

beforeEach(() => {
  undoable.length = 0;
  installExecCommand();
});
afterEach(() => {
  cleanup();
  document.body.innerHTML = '';
});

describe('formatInsertion', () => {
  it.each([
    ['', 'hello', 'Hello'],
    ['   ', 'hello', 'Hello'],
    ['Hi', 'there', ' there'],
    ['Hi ', 'there', 'there'],
    ['Done.', 'next', ' Next'],
    ['Done! ', 'next', 'Next'],
    ['Really?\n', 'yes', 'Yes'],
    ['a, b', 'c', ' c'],
    ['', '«привет»', '«Привет»'],
    ['', '3 cats', '3 cats'],
    ['Hi', 'Valera, hi.', ' Valera, hi.'],
  ])('after %j inserts %j as %j', (before, text, expected) => {
    expect(formatInsertion(before, text)).toBe(expected);
  });

  it.each([
    ['', 'Hello'],
    ['Note:', 'Hello Note:'],
    ['(x)', 'Hello (x)'],
    ['.', 'Hello.'],
    [', and', 'Hello, and'],
    ['!', 'Hello!'],
    ['?', 'Hello?'],
    [';', 'Hello;'],
    [':', 'Hello:'],
    [')', 'Hello)'],
    [']', 'Hello]'],
    ['»', 'Hello»'],
    ['”', 'Hello”'],
    [' rest', 'Hello rest'],
    ['\nrest', 'Hello\nrest'],
  ])('before %j adds a space after the text only when needed: %j', (after, expected) => {
    expect(formatInsertion('', 'hello', after) + after).toBe(expected);
  });
});

describe('FieldBinding', () => {
  it('inserts at the caret and continues after the committed text', () => {
    const { textarea, binding } = field('Hello world');
    textarea.setSelectionRange(5, 5);
    binding.begin();
    binding.commit('there');
    binding.commit('my');
    expect(textarea.value).toBe('Hello there my world');
  });

  it('adds a space after text inserted before a word, in the draft and in the final (regression: "today!Note:")', () => {
    const { textarea, binding } = field('Note: buy milk');
    textarea.setSelectionRange(0, 0);
    binding.begin();
    binding.draft('happy birthday');
    expect(textarea.value).toBe('Happy birthday Note: buy milk');
    binding.commit('Happy birthday, Valera. The weather is lovely today!');
    expect(textarea.value).toBe('Happy birthday, Valera. The weather is lovely today! Note: buy milk');
    binding.commit('Second part.');
    expect(textarea.value).toBe('Happy birthday, Valera. The weather is lovely today! Second part. Note: buy milk');
  });

  it('adds no space after text inserted before attaching punctuation', () => {
    const { textarea, binding } = field('Hi, Valera');
    textarea.setSelectionRange(2, 2);
    binding.begin();
    binding.commit('there');
    expect(textarea.value).toBe('Hi there, Valera');
  });

  it('replaces the selection', () => {
    const { textarea, binding } = field('Hello world');
    textarea.setSelectionRange(6, 11);
    binding.begin();
    binding.draft('fri');
    expect(textarea.value).toBe('Hello fri');
    binding.commit('friend');
    expect(textarea.value).toBe('Hello friend');
  });

  it('puts the selection back when the segment turns out empty', () => {
    const { textarea, binding } = field('Hello world');
    textarea.setSelectionRange(6, 11);
    binding.begin();
    binding.draft('fri');
    binding.clear();
    expect(textarea.value).toBe('Hello world');
  });

  it('goes to the end of the text when the field is not focused', () => {
    const { textarea, binding } = field('Note:', false);
    textarea.setSelectionRange(0, 0);
    binding.begin();
    binding.commit('buy milk');
    expect(textarea.value).toBe('Note: buy milk');
    expect(undoable).toEqual([]); // no focus: the value setter, not the undo stack
  });

  it('replaces the draft in place on every draft and with the final text', () => {
    const { textarea, binding, inputs } = field('Start. end');
    textarea.setSelectionRange(6, 6);
    binding.begin();
    binding.draft('hel');
    expect(textarea.value).toBe('Start. Hel end');
    binding.draft('hello wor');
    expect(textarea.value).toBe('Start. Hello wor end');
    binding.commit('hello world.');
    expect(textarea.value).toBe('Start. Hello world. end');
    expect(inputs).toHaveBeenCalledTimes(3); // two drafts and the commit
  });

  it('commits through insertText while focused, and drafts never reach the undo stack', () => {
    const { textarea, binding } = field('');
    binding.begin();
    binding.draft('one');
    binding.draft('one two');
    binding.commit('one two.');
    binding.draft('three');
    binding.commit('three.');
    expect(textarea.value).toBe('One two. Three.');
    expect(undoable).toEqual(['One two.', ' Three.']);
  });

  it('drops the draft on a user edit outside it, and the next commit goes to the caret', () => {
    const { textarea, binding } = field('abc');
    binding.begin();
    binding.draft('hello');
    expect(textarea.value).toBe('abc hello');
    userTypes(textarea, 0, 'X');
    expect(textarea.value).toBe('Xabc');
    binding.draft('hello again'); // not re-inserted
    expect(textarea.value).toBe('Xabc');
    textarea.setSelectionRange(1, 1);
    binding.commit('hello again');
    expect(textarea.value).toBe('X hello again abc');
  });

  it('leaves the text to the user after an edit inside the draft, and skips the final of that segment', () => {
    const { textarea, binding } = field('');
    binding.begin();
    binding.draft('helo');
    userTypes(textarea, 3, 'l');
    expect(textarea.value).toBe('Hello');
    binding.draft('hello wor'); // later drafts of the segment stay hidden
    expect(textarea.value).toBe('Hello');
    textarea.setSelectionRange(5, 5);
    binding.commit('hello world'); // not inserted, so the text is not repeated
    expect(textarea.value).toBe('Hello');
    binding.commit('next one'); // the next segment goes to the caret again
    expect(textarea.value).toBe('Hello next one');
  });

  it('sees a change of the value from outside before it writes', () => {
    const { textarea, binding } = field('draft me');
    binding.begin();
    binding.commit('one');
    textarea.value = ''; // the host cleared the field, without an input event
    binding.commit('two');
    expect(textarea.value).toBe('Two');
  });

  it('calls onEscape on Esc and prevents the default when it stops', () => {
    const { textarea, onEscape } = field('');
    const event = new KeyboardEvent('keydown', { key: 'Escape', cancelable: true });
    textarea.dispatchEvent(event);
    expect(onEscape).toHaveBeenCalledOnce();
    expect(event.defaultPrevented).toBe(true);
  });

  it('takes the draft out of the field when it detaches', () => {
    const { textarea, binding, detach } = field('a');
    binding.begin();
    binding.draft('b');
    detach();
    expect(textarea.value).toBe('a');
  });
});

describe('useDictation', () => {
  const LOUD = 0.1;
  const feed = (rms: number, blocks: number) =>
    act(() => {
      for (let i = 0; i < blocks; i++) mic.onBlock?.({ pcm: new Float32Array(320).fill(rms), rms, ms: 20 });
    });

  function Composer({ engine }: { engine: { load: () => Promise<void>; transcribe: () => Promise<string> } }) {
    const ref = useRef<HTMLTextAreaElement>(null);
    const [text, setText] = useState('Hi');
    const listener = useDictation(ref, { lang: 'en', engine, interimEveryMs: 1000 });
    return (
      <>
        <textarea aria-label="Message" ref={ref} value={text} onChange={(e) => setText(e.target.value)} />
        <output data-testid="state">{text}</output>
        <MicButton listener={listener} />
        <ListenerPill listener={listener} />
      </>
    );
  }

  it('writes drafts and segments into a controlled textarea, not into the pill, and keeps the focus there', async () => {
    vi.useFakeTimers();
    try {
      let answer = 'there';
      const engine = { load: vi.fn(async () => undefined), transcribe: vi.fn(async () => answer) };
      const { getByLabelText, getByRole, getByTestId, container } = render(<Composer engine={engine} />);
      const textarea = getByLabelText('Message') as HTMLTextAreaElement;
      textarea.focus();
      textarea.setSelectionRange(2, 2);

      const micButton = getByRole('button', { name: /start/i });
      // The press must not take the focus from the field.
      expect(fireEvent.mouseDown(micButton)).toBe(false);
      fireEvent.click(micButton);
      await act(() => vi.advanceTimersByTimeAsync(0));
      expect(micButton.getAttribute('data-state')).toBe('listening');

      feed(LOUD, 30);
      await act(() => vi.advanceTimersByTimeAsync(1000));
      expect(textarea.value).toBe('Hi there');
      expect(getByTestId('state').textContent).toBe('Hi there'); // React saw the input event
      expect(container.querySelector('.hbv-pill__text')).toBeNull();

      answer = 'there, Valera.';
      feed(0.001, 35);      await act(() => vi.advanceTimersByTimeAsync(0));
      expect(textarea.value).toBe('Hi there, Valera.');
      expect(getByTestId('state').textContent).toBe('Hi there, Valera.');
      expect(undoable).toEqual([' there, Valera.']);

      fireEvent.keyDown(textarea, { key: 'Escape' });
      await act(() => vi.advanceTimersByTimeAsync(0));
      expect(micButton.getAttribute('data-state')).toBe('idle');
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('MicButton hold', () => {
  // jsdom has no PointerEvent; without it fireEvent sends a plain Event that has no `button`.
  beforeEach(() => {
    class PointerEventStub extends MouseEvent {
      readonly pointerId: number;
      constructor(type: string, init: PointerEventInit = {}) {
        super(type, init);
        this.pointerId = init.pointerId ?? 0;
      }
    }
    vi.stubGlobal('PointerEvent', PointerEventStub);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('renders data-hold while the pointer holds the mic, and stops on release', async () => {
    vi.useFakeTimers();
    try {
      const engine = { load: vi.fn(async () => undefined), transcribe: vi.fn(async () => 'hi') };
      function Holder() {
        const ref = useRef<HTMLTextAreaElement>(null);
        const listener = useDictation(ref, { lang: 'en', engine });
        return <MicButton listener={listener} />;
      }
      const { getByRole } = render(<Holder />);
      const button = getByRole('button');
      fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
      await act(() => vi.advanceTimersByTimeAsync(300));
      expect(button.getAttribute('data-hold')).toBe('true');
      expect(button.getAttribute('data-state')).toBe('listening');
      fireEvent.pointerUp(button, { button: 0, pointerId: 1 });
      fireEvent.click(button, { detail: 1 });
      await act(() => vi.advanceTimersByTimeAsync(0));
      expect(button.getAttribute('data-hold')).toBe('false');
      expect(button.getAttribute('data-state')).toBe('idle');
    } finally {
      vi.useRealTimers();
    }
  });
});
