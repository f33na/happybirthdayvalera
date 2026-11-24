import type { DictationSink } from './controller';

export type TextField = HTMLTextAreaElement | HTMLInputElement;

/** Punctuation that belongs to the word on its left, so no space goes before it. */
const ATTACHES_LEFT = /^[.,!?;:)\]»”]/;

/**
 * The text to insert between `before` and `after`: one space before it when the previous character is not
 * whitespace, one space after it when the next character is neither whitespace nor punctuation that attaches to
 * the left, and a capital first letter at the start of the field or after `.`, `!` or `?`. Whisper's own
 * punctuation stays as it is.
 */
export function formatInsertion(before: string, text: string, after = ''): string {
  const space = before.length > 0 && !/\s$/.test(before) ? ' ' : '';
  const trailing = after.length > 0 && !/^\s/.test(after) && !ATTACHES_LEFT.test(after) ? ' ' : '';
  const sentenceStart = /^\s*$/.test(before) || /[.!?]\s*$/.test(before);
  const body = sentenceStart ? text.replace(/^([^\p{L}\p{N}]*)(\p{L})/u, (_m, lead: string, letter: string) => lead + letter.toUpperCase()) : text;
  return space + body + trailing;
}

/** Where a user edit changed `before` into `after`: [from, to) in `before`. */
function editRange(before: string, after: string): { from: number; to: number } {
  const max = Math.min(before.length, after.length);
  let prefix = 0;
  while (prefix < max && before[prefix] === after[prefix]) prefix++;
  let suffix = 0;
  while (suffix < max - prefix && before[before.length - 1 - suffix] === after[after.length - 1 - suffix]) suffix++;
  return { from: prefix, to: before.length - suffix };
}

/**
 * Writes dictation into a `<textarea>` or `<input>` (see CONTRACT.md, "Dictation into a text field").
 * The draft is plain text in the field, written with the native value setter, so it never enters the undo
 * stack. A final segment goes in with `execCommand('insertText')` while the field has focus, so one Cmd/Ctrl+Z
 * removes it. Every change fires a native `input` event, so controlled inputs see it.
 */
export class FieldBinding implements DictationSink {
  private field: TextField | null = null;
  /** The field value without the draft. */
  private base = '';
  /** The range of `base` that the next insertion replaces; wider than empty only for the starting selection. */
  private start = 0;
  private end = 0;
  /** The draft as it is shown, with its spaces; null while none is shown. */
  private shown: string | null = null;
  /** The value we wrote last. Anything else in the field is a user edit. */
  private expected = '';
  /** After a user edit, the next text goes to the caret. */
  private followCaret = false;
  /** The user edited the field while a draft was shown: no more drafts until the segment ends. */
  private muted = false;
  /** The user edited inside the draft, so the draft text is theirs now: the final of that segment is skipped. */
  private skipFinal = false;
  private writing = false;

  /** Starts listening to the field. Returns the function that stops it. */
  attach(field: TextField, onEscape: () => boolean): () => void {
    this.field = field;
    this.base = field.value;
    this.expected = field.value;
    const onInput = (): void => {
      if (!this.writing) this.userEdit();
    };
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key === 'Escape' && onEscape()) event.preventDefault();
    };
    // The union type of `field` hides the typed overloads of addEventListener; the common base has them.
    const element: HTMLElement = field;
    element.addEventListener('input', onInput);
    element.addEventListener('keydown', onKeyDown);
    return () => {
      this.clear();
      element.removeEventListener('input', onInput);
      element.removeEventListener('keydown', onKeyDown);
      if (this.field === field) this.field = null;
    };
  }

  begin(): void {
    const field = this.field;
    if (!field) return;
    this.base = field.value;
    this.expected = field.value;
    this.shown = null;
    this.muted = false;
    this.skipFinal = false;
    this.followCaret = false;
    const focused = field.ownerDocument.activeElement === field;
    const caret = focused ? field.selectionStart : null;
    if (caret === null) {
      this.start = this.end = field.value.length;
    } else {
      this.start = caret;
      this.end = field.selectionEnd ?? caret;
    }
  }

  draft(text: string): void {
    const field = this.sync();
    if (!field || this.muted) return;
    this.takeCaret(field);
    const shown = formatInsertion(this.base.slice(0, this.start), text, this.base.slice(this.end));
    this.write(field, this.base.slice(0, this.start) + shown + this.base.slice(this.end), this.start + shown.length, true);
    this.shown = shown;
  }

  commit(text: string): void {
    this.muted = false;
    const field = this.sync();
    if (!field) return;
    if (this.skipFinal) {
      this.skipFinal = false;
      return;
    }
    this.takeCaret(field);
    const inserted = formatInsertion(this.base.slice(0, this.start), text, this.base.slice(this.end));
    const next = this.base.slice(0, this.start) + inserted + this.base.slice(this.end);
    // Put the field back to its value without the draft, quietly: the insert below fires the one input event.
    if (this.shown !== null) this.write(field, this.base, null, false);
    this.shown = null;
    let done = false;
    const doc = field.ownerDocument;
    // `execCommand` is deprecated, but it is still the only way to put an edit on the browser's own undo stack.
    if (doc.activeElement === field && typeof doc.execCommand === 'function') {
      this.writing = true;
      try {
        field.setSelectionRange(this.start, this.end);
        done = doc.execCommand('insertText', false, inserted);
      } finally {
        this.writing = false;
      }
    }
    // No focus, or a browser without insertText: the value setter works, but the browser cannot undo it.
    if (!done) this.write(field, next, this.start + inserted.length, true);
    this.base = field.value;
    this.expected = field.value;
    this.start = this.end = Math.min(this.start + inserted.length, this.base.length);
  }

  clear(): void {
    this.muted = false;
    this.skipFinal = false;
    const field = this.sync();
    if (!field || this.shown === null) return;
    this.shown = null;
    this.write(field, this.base, this.start, true);
  }

  /** The field, after taking in any change that we did not make. */
  private sync(): TextField | null {
    const field = this.field;
    if (field && field.value !== this.expected) this.userEdit();
    return field;
  }

  private takeCaret(field: TextField): void {
    if (!this.followCaret) return;
    this.followCaret = false;
    this.start = this.end = Math.min(field.selectionStart ?? this.base.length, this.base.length);
  }

  private userEdit(): void {
    const field = this.field;
    if (!field) return;
    const now = field.value;
    const shown = this.shown;
    if (shown !== null) {
      this.shown = null;
      this.muted = true;
      const { from, to } = editRange(this.expected, now);
      const draftEnd = this.start + shown.length;
      let at: number | null = null;
      if (to <= this.start) at = this.start + now.length - this.expected.length;
      else if (from >= draftEnd) at = this.start;
      // An edit inside the draft makes it the user's text; anywhere else, the draft comes out of the field.
      if (at === null) this.skipFinal = true;
      else {
        const caret = field.selectionStart ?? now.length;
        const kept = caret > at ? caret - Math.min(shown.length, caret - at) : caret;
        this.write(field, now.slice(0, at) + now.slice(at + shown.length), kept, true);
      }
    }
    this.base = field.value;
    this.expected = field.value;
    this.followCaret = true;
  }

  private write(field: TextField, value: string, caret: number | null, notify: boolean): void {
    this.writing = true;
    try {
      // The prototype setter, not `field.value =`: React tracks the instance property, so a write through it
      // would hide the change from the next input event of a controlled field.
      const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(field), 'value')?.set;
      if (setter) setter.call(field, value);
      else field.value = value;
      this.expected = field.value;
      if (caret !== null && field.ownerDocument.activeElement === field) field.setSelectionRange(caret, caret);
      if (notify) field.dispatchEvent(new Event('input', { bubbles: true }));
    } finally {
      this.writing = false;
    }
  }
}
