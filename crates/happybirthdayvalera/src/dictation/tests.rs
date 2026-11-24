//! The insertion rules of [`super::use_dictation`], over a fake field.

use super::field::{Edit, FieldBinding, FieldEvent, Snapshot, format_insert};

/// A field that applies edits like the ports do: only while it still holds `expect`.
struct FakeField {
    value: String,
    sel: (usize, usize),
    focused: bool,
    log: Vec<Edit>,
}

impl FakeField {
    fn new(value: &str) -> Self {
        let end = value.encode_utf16().count();
        Self { value: value.into(), sel: (end, end), focused: false, log: Vec::new() }
    }

    fn focus(mut self, start: usize, end: usize) -> Self {
        self.focused = true;
        self.sel = (start, end);
        self
    }

    fn snap(&self) -> Snapshot {
        Snapshot { value: self.value.clone(), sel_start: self.sel.0, sel_end: self.sel.1, focused: self.focused }
    }

    /// The user types: replaces the whole value and puts the caret at `caret`.
    fn user(&mut self, value: &str, caret: usize) {
        self.value = value.into();
        self.sel = (caret, caret);
    }
}

fn send(b: &mut FieldBinding, f: &mut FakeField, event: FieldEvent) {
    for edit in b.handle(event, Some(f.snap())) {
        let result = (f.value == edit.expect).then(|| {
            f.value.clone_from(&edit.after);
            f.sel = (edit.caret, edit.caret);
            f.value.clone()
        });
        f.log.push(edit);
        if !b.applied(result) {
            break;
        }
    }
}

fn draft(segment: u32, text: &str) -> FieldEvent {
    FieldEvent::Draft { segment, text: text.into() }
}

fn commit(segment: u32, text: &str) -> FieldEvent {
    FieldEvent::Commit { segment, text: text.into() }
}

#[test]
fn spacing_and_capitalization() {
    assert_eq!(format_insert("", "", "hello"), "Hello");
    assert_eq!(format_insert("   ", "", "hello"), "Hello");
    assert_eq!(format_insert("Hi", "", "there"), " there");
    assert_eq!(format_insert("Hi ", "", "there"), "there");
    assert_eq!(format_insert("Hi.", "", "next one"), " Next one");
    assert_eq!(format_insert("Really?\n", "", "yes"), "Yes");
    assert_eq!(format_insert("Wow!", "", "«ура»"), " «Ура»");
    assert_eq!(format_insert("and", "", "Whisper's Case, kept."), " Whisper's Case, kept.");
    assert_eq!(format_insert("", "", "  трим  "), "Трим");
}

#[test]
fn a_space_goes_after_the_text_unless_whitespace_or_attaching_punctuation_follows() {
    assert_eq!(format_insert("", "Note:", "hello"), "Hello ");
    assert_eq!(format_insert("a ", " b", "x"), "x");
    assert_eq!(format_insert("a ", "\nb", "x"), "x");
    for p in [".", ",", "!", "?", ";", ":", ")", "]", "»", "\u{201d}"] {
        assert_eq!(format_insert("a ", p, "x"), "x", "before {p}");
    }
    assert_eq!(format_insert("a ", "(b", "x"), "x ", "an opening bracket gets a space");
    assert_eq!(format_insert("a ", "«b", "x"), "x ");
}

#[test]
fn inserting_mid_text_keeps_words_apart() {
    let mut f = FakeField::new("Hello world").focus(6, 6);
    let mut b = FieldBinding::default();
    send(&mut b, &mut f, FieldEvent::Start);
    send(&mut b, &mut f, draft(0, "big"));
    assert_eq!(f.value, "Hello big world");
    send(&mut b, &mut f, commit(0, "big"));
    assert_eq!(f.value, "Hello big world");
    send(&mut b, &mut f, commit(1, "wide"));
    assert_eq!(f.value, "Hello big wide world", "the next segment goes after the space, before the word");
}

#[test]
fn inserts_at_the_caret_of_a_focused_field() {
    let mut f = FakeField::new("Hello world").focus(5, 5);
    let mut b = FieldBinding::default();
    send(&mut b, &mut f, FieldEvent::Start);
    send(&mut b, &mut f, commit(0, "there"));
    assert_eq!(f.value, "Hello there world");
    assert_eq!(f.sel, (11, 11), "the caret follows the inserted text");
}

#[test]
fn inserts_at_the_end_of_a_field_without_focus() {
    let mut f = FakeField::new("Hello world.");
    f.sel = (0, 0);
    let mut b = FieldBinding::default();
    send(&mut b, &mut f, FieldEvent::Start);
    send(&mut b, &mut f, commit(0, "bye"));
    assert_eq!(f.value, "Hello world. Bye");
}

#[test]
fn replaces_the_selection() {
    let mut f = FakeField::new("I like cats a lot.").focus(7, 11);
    let mut b = FieldBinding::default();
    send(&mut b, &mut f, FieldEvent::Start);
    send(&mut b, &mut f, draft(0, "do"));
    assert_eq!(f.value, "I like do a lot.");
    send(&mut b, &mut f, commit(0, "dogs"));
    assert_eq!(f.value, "I like dogs a lot.");
}

#[test]
fn an_empty_segment_puts_the_selection_back() {
    let mut f = FakeField::new("I like cats.").focus(7, 11);
    let mut b = FieldBinding::default();
    send(&mut b, &mut f, FieldEvent::Start);
    send(&mut b, &mut f, draft(0, "uh"));
    assert_eq!(f.value, "I like uh.");
    send(&mut b, &mut f, draft(0, ""));
    assert_eq!(f.value, "I like cats.");
    send(&mut b, &mut f, FieldEvent::End);
    assert_eq!(f.value, "I like cats.");
}

#[test]
fn the_draft_is_replaced_in_place_and_the_next_segment_follows() {
    let mut f = FakeField::new("").focus(0, 0);
    let mut b = FieldBinding::default();
    send(&mut b, &mut f, FieldEvent::Start);
    send(&mut b, &mut f, draft(0, "hel"));
    assert_eq!(f.value, "Hel");
    send(&mut b, &mut f, draft(0, "hello wor"));
    assert_eq!(f.value, "Hello wor");
    send(&mut b, &mut f, commit(0, "hello world."));
    assert_eq!(f.value, "Hello world.");
    send(&mut b, &mut f, draft(1, "how are"));
    assert_eq!(f.value, "Hello world. How are");
    send(&mut b, &mut f, commit(1, "how are you?"));
    assert_eq!(f.value, "Hello world. How are you?");

    let kinds: Vec<(bool, usize, usize)> = f.log.iter().map(|e| (e.commit, e.start, e.end)).collect();
    assert_eq!(
        kinds,
        [(false, 0, 0), (false, 0, 3), (true, 0, 9), (false, 12, 12), (true, 12, 20)],
        "drafts use the setter; only finals are commits, each replacing the draft region"
    );
}

#[test]
fn a_user_edit_elsewhere_drops_the_draft_and_the_final_goes_to_the_caret() {
    let mut f = FakeField::new("Note: ").focus(6, 6);
    let mut b = FieldBinding::default();
    send(&mut b, &mut f, FieldEvent::Start);
    send(&mut b, &mut f, draft(0, "buy mi"));
    assert_eq!(f.value, "Note: buy mi");
    // The user types at the very start.
    f.user("A Note: buy mi", 2);
    send(&mut b, &mut f, draft(0, "buy milk"));
    assert_eq!(f.value, "A Note: ", "the intact draft is taken out and not shown again");
    send(&mut b, &mut f, commit(0, "buy milk."));
    assert_eq!(f.value, "A buy milk. Note: ", "the final goes to the caret");
}

#[test]
fn a_user_edit_inside_the_draft_keeps_it_and_skips_its_final() {
    let mut f = FakeField::new("").focus(0, 0);
    let mut b = FieldBinding::default();
    send(&mut b, &mut f, FieldEvent::Start);
    send(&mut b, &mut f, draft(0, "helo"));
    assert_eq!(f.value, "Helo");
    f.user("Hello", 5);
    send(&mut b, &mut f, draft(0, "hello"));
    send(&mut b, &mut f, commit(0, "hello."));
    assert_eq!(f.value, "Hello", "the user's text stays and is not repeated");
    send(&mut b, &mut f, commit(1, "next."));
    assert_eq!(f.value, "Hello next.", "later segments go to the caret");
}

#[test]
fn a_user_edit_between_segments_moves_to_the_caret() {
    let mut f = FakeField::new("").focus(0, 0);
    let mut b = FieldBinding::default();
    send(&mut b, &mut f, FieldEvent::Start);
    send(&mut b, &mut f, commit(0, "one."));
    f.user("Zero. One.", 0);
    send(&mut b, &mut f, commit(1, "two."));
    assert_eq!(f.value, "Two. Zero. One.");
}

#[test]
fn a_refused_edit_counts_as_a_user_edit() {
    let mut f = FakeField::new("").focus(0, 0);
    let mut b = FieldBinding::default();
    send(&mut b, &mut f, FieldEvent::Start);
    send(&mut b, &mut f, draft(0, "hi"));
    let mut edits = b.handle(draft(0, "hi there"), Some(f.snap()));
    assert_eq!(edits.len(), 1);
    // The user typed between the read and the write: the port refuses.
    f.user("Hi!", 3);
    assert!(!b.applied(None));
    edits.clear();
    send(&mut b, &mut f, commit(0, "hi there."));
    assert_eq!(f.value, "Hi! Hi there.");
}

#[test]
fn the_end_takes_out_a_leftover_draft() {
    let mut f = FakeField::new("Text").focus(4, 4);
    let mut b = FieldBinding::default();
    send(&mut b, &mut f, FieldEvent::Start);
    send(&mut b, &mut f, draft(0, "more"));
    assert_eq!(f.value, "Text more");
    send(&mut b, &mut f, FieldEvent::End);
    assert_eq!(f.value, "Text");
    send(&mut b, &mut f, commit(1, "late"));
    assert_eq!(f.value, "Text", "no binding after the end");
}

#[test]
fn offsets_count_utf16_units() {
    // "👋" is two UTF-16 units; the caret sits after it.
    let mut f = FakeField::new("Привет 👋 мир").focus(9, 9);
    let mut b = FieldBinding::default();
    send(&mut b, &mut f, FieldEvent::Start);
    send(&mut b, &mut f, commit(0, "и всем"));
    assert_eq!(f.value, "Привет 👋 и всем мир");
}

#[test]
fn a_missing_field_does_nothing() {
    let mut b = FieldBinding::default();
    assert!(b.handle(FieldEvent::Start, None).is_empty());
    assert!(b.handle(commit(0, "x"), None).is_empty());
}
