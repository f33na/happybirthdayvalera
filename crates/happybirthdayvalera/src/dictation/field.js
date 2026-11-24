// The bound text field for renderers without web-sys (desktop). Rust prepends HBV_FIELD (the element id) and
// HBV_MODE: "field" answers {op: "read"} and {op: "apply", edit} with one reply each; "escape" reports Esc keydowns.

function hbvField() {
  let el = document.getElementById(HBV_FIELD);
  if (el && el.tagName !== 'TEXTAREA' && el.tagName !== 'INPUT') el = el.querySelector('textarea, input');
  return el;
}

function hbvSnapshot(el) {
  const len = el.value.length;
  return {
    value: el.value,
    start: el.selectionStart ?? len,
    end: el.selectionEnd ?? len,
    focused: document.activeElement === el,
  };
}

function hbvSelect(el, start, end) {
  // Input types without a selection (email, number) throw; the text is in place all the same.
  try { el.setSelectionRange(start, end); } catch (_) { /* see above */ }
}

function hbvApply(el, e) {
  if (el.value !== e.expect) return null;
  if (e.commit && document.activeElement === el) {
    hbvSelect(el, e.start, e.end);
    // On the undo stack, and the browser fires input itself.
    if (document.execCommand('insertText', false, e.text)) return el.value;
  }
  el.value = e.after;
  hbvSelect(el, e.caret, e.caret);
  el.dispatchEvent(new Event('input', { bubbles: true }));
  return el.value;
}

if (HBV_MODE === 'escape') {
  const onKey = (e) => {
    if (e.key !== 'Escape' || e.target !== hbvField()) return;
    try {
      dioxus.send(true);
    } catch (_) {
      // The Rust side is gone (the component unmounted).
      document.removeEventListener('keydown', onKey, true);
    }
  };
  document.addEventListener('keydown', onKey, true);
  await new Promise(() => {});
} else {
  for (;;) {
    const m = await dioxus.recv();
    const el = hbvField();
    if (!el) dioxus.send(null);
    else if (m.op === 'read') dioxus.send(hbvSnapshot(el));
    else dioxus.send(hbvApply(el, m.edit));
  }
}
