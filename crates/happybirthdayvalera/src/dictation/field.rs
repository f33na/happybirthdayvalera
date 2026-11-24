//! The pure insertion rules of a bound text field. Offsets count UTF-16 code units, as the DOM does.

/// The field as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct Snapshot {
    pub(crate) value: String,
    #[serde(rename = "start")]
    pub(crate) sel_start: usize,
    #[serde(rename = "end")]
    pub(crate) sel_end: usize,
    pub(crate) focused: bool,
}

/// One change: replace `expect[start..end]` with `text`. A port applies it only while the field still holds
/// `expect`; `after` is the value it then holds, and the caret goes to `start + len(text)`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct Edit {
    pub(crate) expect: String,
    pub(crate) after: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) text: String,
    pub(crate) caret: usize,
    /// A final segment: the port inserts it with `execCommand('insertText')` when the field has focus, so one
    /// undo step removes it. Drafts use the value setter and stay off the undo stack.
    pub(crate) commit: bool,
}

/// What the listener tells the bound field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FieldEvent {
    /// A session started (`START` was accepted).
    Start,
    /// The draft of `segment`; empty clears it.
    Draft { segment: u32, text: String },
    /// The final text of `segment`.
    Commit { segment: u32, text: String },
    /// The session ended.
    End,
}

/// What sits at the insertion point and belongs to the binding.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Region {
    Empty,
    /// The user's selection from the session start; the first insertion replaces it.
    Selection(String),
    /// A shown draft. `restore` is the selection it replaced, put back if the segment turns out empty.
    Draft { segment: u32, len: usize, restore: Option<String> },
}

impl Region {
    fn len(&self) -> usize {
        match self {
            Region::Empty => 0,
            Region::Selection(s) => len16(s),
            Region::Draft { len, .. } => *len,
        }
    }
}

#[derive(Debug)]
struct Bound {
    /// The value after the last write or read of the binding; any other value means the user edited.
    value: String,
    anchor: usize,
    region: Region,
    /// The segment whose draft the user edited away: its drafts stay out of the field, and its final too when
    /// `skip_final` (the user changed the draft text itself, so the final would repeat it).
    dropped: Option<(u32, bool)>,
}

/// The binding state of one field across a session.
#[derive(Debug, Default)]
pub(crate) struct FieldBinding {
    bound: Option<Bound>,
}

impl FieldBinding {
    /// The edits for `event` over the field as it is now (`None`: no such field).
    pub(crate) fn handle(&mut self, event: FieldEvent, snap: Option<Snapshot>) -> Vec<Edit> {
        let Some(snap) = snap else {
            self.bound = None;
            return Vec::new();
        };
        match event {
            FieldEvent::Start => {
                self.start(snap);
                Vec::new()
            }
            FieldEvent::Draft { segment, text } => self.with(&snap, |b, edits| b.draft(segment, &text, edits)),
            FieldEvent::Commit { segment, text } => self.with(&snap, |b, edits| b.commit(segment, &text, edits)),
            FieldEvent::End => {
                let edits = self.with(&snap, Bound::clear);
                self.bound = None;
                edits
            }
        }
    }

    /// The port applied the last edit (`Some`: the value the field now holds) or refused it (`None`: the
    /// field changed under it). Returns false when the remaining edits must not be applied.
    pub(crate) fn applied(&mut self, result: Option<String>) -> bool {
        let Some(b) = &mut self.bound else { return false };
        if let Some(value) = result {
            b.value = value;
            return true;
        }
        // The user's change wins; the next event sees it as an edit and moves to the caret.
        b.region = Region::Empty;
        b.value = String::new();
        false
    }

    fn start(&mut self, snap: Snapshot) {
        let len = len16(&snap.value);
        let (anchor, region) = if snap.focused {
            let start = snap.sel_start.min(len);
            let end = snap.sel_end.clamp(start, len);
            let region =
                if end > start { Region::Selection(slice16(&snap.value, start, end).to_owned()) } else { Region::Empty };
            (start, region)
        } else {
            (len, Region::Empty)
        };
        self.bound = Some(Bound { value: snap.value, anchor, region, dropped: None });
    }

    fn with(&mut self, snap: &Snapshot, f: impl FnOnce(&mut Bound, &mut Vec<Edit>)) -> Vec<Edit> {
        let Some(b) = &mut self.bound else { return Vec::new() };
        let mut edits = Vec::new();
        b.sync(snap, &mut edits);
        f(b, &mut edits);
        edits
    }
}

impl Bound {
    /// Handles a user edit since the last write: the draft is dropped, and the insertion point moves to the
    /// caret (the end when the field has no focus).
    fn sync(&mut self, snap: &Snapshot, edits: &mut Vec<Edit>) {
        if snap.value == self.value {
            return;
        }
        let mut value = snap.value.clone();
        let mut caret = if snap.focused { snap.sel_end.min(len16(&value)) } else { len16(&value) };
        if let Region::Draft { segment, len, .. } = self.region {
            let (p, old_end, new_end) = diff16(&self.value, &value);
            let (a, z) = (self.anchor, self.anchor + len);
            let touched = if old_end == p { p > a && p < z } else { p < z && old_end > a };
            if touched {
                self.dropped = Some((segment, true));
            } else {
                // The edit was elsewhere: the draft is intact, so take it out again.
                let start = if old_end <= a { a + new_end - old_end } else { a };
                let after = splice16(&value, start, start + len, "");
                edits.push(Edit {
                    expect: value,
                    after: after.clone(),
                    start,
                    end: start + len,
                    text: String::new(),
                    caret: start,
                    commit: false,
                });
                if caret >= start + len {
                    caret -= len;
                } else if caret > start {
                    caret = start;
                }
                value = after;
                self.dropped = Some((segment, false));
            }
        }
        self.value = value;
        self.anchor = caret;
        self.region = Region::Empty;
    }

    fn draft(&mut self, segment: u32, text: &str, edits: &mut Vec<Edit>) {
        if self.dropped.is_some_and(|(s, _)| s == segment) {
            return;
        }
        if text.trim().is_empty() {
            return self.clear(edits);
        }
        let old_len = self.region.len();
        let formatted = self.format(old_len, text);
        let restore = match std::mem::replace(&mut self.region, Region::Empty) {
            Region::Empty => None,
            Region::Selection(s) => Some(s),
            Region::Draft { restore, .. } => restore,
        };
        self.replace(old_len, &formatted, false, edits);
        self.region = Region::Draft { segment, len: len16(&formatted), restore };
    }

    fn commit(&mut self, segment: u32, text: &str, edits: &mut Vec<Edit>) {
        if self.dropped.is_some_and(|(s, skip_final)| s == segment && skip_final) {
            return;
        }
        if text.trim().is_empty() {
            return self.clear(edits);
        }
        let formatted = self.format(self.region.len(), text);
        self.replace(self.region.len(), &formatted, true, edits);
        self.anchor += len16(&formatted);
        self.region = Region::Empty;
    }

    /// Takes a shown draft out, putting back the selection it replaced.
    fn clear(&mut self, edits: &mut Vec<Edit>) {
        let Region::Draft { restore, .. } = &self.region else { return };
        let restore = restore.clone();
        self.replace(self.region.len(), restore.as_deref().unwrap_or(""), false, edits);
        self.region = restore.map_or(Region::Empty, Region::Selection);
    }

    /// `text` formatted for the insertion point, with the `old_len` units there replaced.
    fn format(&self, old_len: usize, text: &str) -> String {
        let before = slice16(&self.value, 0, self.anchor);
        let after = slice16(&self.value, self.anchor + old_len, len16(&self.value));
        format_insert(before, after, text)
    }

    /// Replaces the `old_len` units at the insertion point with `text`; the caller sets the new region.
    fn replace(&mut self, old_len: usize, text: &str, commit: bool, edits: &mut Vec<Edit>) {
        let start = self.anchor;
        let end = start + old_len;
        let after = splice16(&self.value, start, end, text);
        edits.push(Edit {
            expect: std::mem::take(&mut self.value),
            after: after.clone(),
            start,
            end,
            text: text.to_owned(),
            caret: start + len16(text),
            commit,
        });
        self.value = after;
    }
}

/// Punctuation that attaches to the word before it, so no space goes between the inserted text and it.
const ATTACHES_LEFT: [char; 9] = ['.', ',', '!', '?', ';', ':', ')', ']', '»'];

/// The text to insert between `before` and `after`: one space before it when the character before is not
/// whitespace, one space after it when the character after is neither whitespace nor punctuation that attaches
/// to the left (`. , ! ? ; : ) ] » ”`), and a capital first letter at the start of the field or after `.`,
/// `!` or `?`. Whisper's punctuation stays as it is.
pub(crate) fn format_insert(before: &str, after: &str, text: &str) -> String {
    let text = text.trim();
    let mut out = String::with_capacity(text.len() + 1);
    if before.chars().next_back().is_some_and(|c| !c.is_whitespace()) {
        out.push(' ');
    }
    let prev = before.trim_end();
    let capitalize = prev.is_empty() || prev.ends_with(['.', '!', '?']);
    let mut done = !capitalize;
    for c in text.chars() {
        if !done && c.is_alphanumeric() {
            done = true;
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
    }
    if after.chars().next().is_some_and(|c| !c.is_whitespace() && !ATTACHES_LEFT.contains(&c) && c != '\u{201d}') {
        out.push(' ');
    }
    out
}

pub(crate) fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The byte index of UTF-16 offset `at`, clamped to the string; an offset inside a surrogate pair rounds down.
fn byte16(s: &str, at: usize) -> usize {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units + c.len_utf16() > at {
            return i;
        }
        units += c.len_utf16();
    }
    s.len()
}

fn slice16(s: &str, start: usize, end: usize) -> &str {
    &s[byte16(s, start)..byte16(s, end)]
}

fn splice16(s: &str, start: usize, end: usize, text: &str) -> String {
    let (a, z) = (byte16(s, start), byte16(s, end));
    let mut out = String::with_capacity(s.len() - (z - a) + text.len());
    out.push_str(&s[..a]);
    out.push_str(text);
    out.push_str(&s[z..]);
    out
}

/// The changed range between `old` and `new`: `(start, old_end, new_end)` in UTF-16 units, from the common
/// prefix and suffix.
fn diff16(old: &str, new: &str) -> (usize, usize, usize) {
    let o: Vec<u16> = old.encode_utf16().collect();
    let n: Vec<u16> = new.encode_utf16().collect();
    let prefix = o.iter().zip(&n).take_while(|(a, b)| a == b).count();
    let room = o.len().min(n.len()) - prefix;
    let suffix = o.iter().rev().zip(n.iter().rev()).take(room).take_while(|(a, b)| a == b).count();
    (prefix, o.len() - suffix, n.len() - suffix)
}
