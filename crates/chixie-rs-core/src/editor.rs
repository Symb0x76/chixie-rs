//! Editable text model behind the WYSIWYG page editor.
//!
//! Text is a `Vec<char>` with a parallel `Vec<u64>` of stable ids: each character keeps the id it
//! was inserted with, and `layout::layout_ids` keys its handwriting randomness by that id, so
//! editing never reshuffles the rest of the page. Positions are char indices (`0..=len`); the
//! IME bridge converts to/from UTF-16 with [`EditDoc::to_utf16`] / [`EditDoc::from_utf16`].

use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    chars: Vec<char>,
    ids: Vec<u64>,
    caret: usize,
    anchor: Option<usize>,
}

/// What the last edit was, to coalesce consecutive typing into one undo step.
#[derive(Debug, Clone, Copy, PartialEq)]
enum LastEdit {
    None,
    Typing,
    Deleting,
}

#[derive(Debug, Clone)]
pub struct EditDoc {
    chars: Vec<char>,
    ids: Vec<u64>,
    next_id: u64,
    caret: usize,
    /// Selection anchor; the selection is between anchor and caret.
    anchor: Option<usize>,
    /// IME composition range (char indices), part of `chars` while composing.
    marked: Option<Range<usize>>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last: LastEdit,
}

const UNDO_LIMIT: usize = 500;

impl Default for EditDoc {
    fn default() -> Self {
        Self::from_parts(Vec::new(), Vec::new())
    }
}

impl EditDoc {
    pub fn from_text(text: &str) -> Self {
        let chars: Vec<char> = text.chars().collect();
        let ids = (0..chars.len() as u64).collect();
        Self::from_parts(chars, ids)
    }

    /// Restore a saved document (ids must match chars one to one; otherwise they are renumbered).
    pub fn from_parts(chars: Vec<char>, ids: Vec<u64>) -> Self {
        let ids = if ids.len() == chars.len() { ids } else { (0..chars.len() as u64).collect() };
        let next_id = ids.iter().max().map_or(0, |m| m + 1);
        let caret = chars.len();
        Self { chars, ids, next_id, caret, anchor: None, marked: None, undo: Vec::new(), redo: Vec::new(), last: LastEdit::None }
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
    }
    pub fn ids(&self) -> &[u64] {
        &self.ids
    }
    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }
    pub fn len(&self) -> usize {
        self.chars.len()
    }
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }
    pub fn caret(&self) -> usize {
        self.caret
    }
    pub fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    /// Selected range (sorted); empty at the caret when nothing is selected.
    pub fn selection(&self) -> Range<usize> {
        match self.anchor {
            Some(a) if a != self.caret => a.min(self.caret)..a.max(self.caret),
            _ => self.caret..self.caret,
        }
    }
    pub fn has_selection(&self) -> bool {
        !self.selection().is_empty()
    }
    /// True when the caret is at the start of the selection (selection made leftwards).
    pub fn selection_reversed(&self) -> bool {
        self.anchor.is_some_and(|a| a > self.caret)
    }
    pub fn selected_text(&self) -> String {
        self.chars[self.selection()].iter().collect()
    }

    /// Move the caret; `extend` keeps/starts a selection from the current anchor.
    pub fn move_to(&mut self, i: usize, extend: bool) {
        let i = i.min(self.len());
        if extend {
            self.anchor.get_or_insert(self.caret);
        } else {
            self.anchor = None;
        }
        self.caret = i;
        self.last = LastEdit::None;
    }

    pub fn select_range(&mut self, r: Range<usize>) {
        let (a, b) = (r.start.min(self.len()), r.end.min(self.len()));
        self.anchor = Some(a);
        self.caret = b;
        self.last = LastEdit::None;
    }

    pub fn select_all(&mut self) {
        self.select_range(0..self.len());
    }

    /// Left/right by one char; with a selection and no `extend`, collapse to its edge.
    pub fn left(&mut self, extend: bool) {
        let sel = self.selection();
        let to = if !extend && !sel.is_empty() { sel.start } else { self.caret.saturating_sub(1) };
        self.move_to(to, extend);
    }
    pub fn right(&mut self, extend: bool) {
        let sel = self.selection();
        let to = if !extend && !sel.is_empty() { sel.end } else { (self.caret + 1).min(self.len()) };
        self.move_to(to, extend);
    }

    /// Word boundaries: CJK characters are one word each; runs of other non-space chars are words.
    pub fn word_left(&self, i: usize) -> usize {
        let mut j = i;
        while j > 0 && self.chars[j - 1].is_whitespace() {
            j -= 1;
        }
        if j > 0 && crate::handwriting::is_cjk(self.chars[j - 1]) {
            return j - 1;
        }
        while j > 0 && !self.chars[j - 1].is_whitespace() && !crate::handwriting::is_cjk(self.chars[j - 1]) {
            j -= 1;
        }
        j
    }
    pub fn word_right(&self, i: usize) -> usize {
        let n = self.len();
        let mut j = i;
        if j < n && crate::handwriting::is_cjk(self.chars[j]) {
            return j + 1;
        }
        while j < n && !self.chars[j].is_whitespace() && !crate::handwriting::is_cjk(self.chars[j]) {
            j += 1;
        }
        while j < n && self.chars[j].is_whitespace() && self.chars[j] != '\n' {
            j += 1;
        }
        j
    }
    /// The word around `i` (for double-click).
    pub fn word_at(&self, i: usize) -> Range<usize> {
        let i = i.min(self.len());
        if i < self.len() && crate::handwriting::is_cjk(self.chars[i]) {
            return i..i + 1;
        }
        let (mut a, mut b) = (i, i);
        while a > 0 && !self.chars[a - 1].is_whitespace() && !crate::handwriting::is_cjk(self.chars[a - 1]) {
            a -= 1;
        }
        while b < self.len() && !self.chars[b].is_whitespace() && !crate::handwriting::is_cjk(self.chars[b]) {
            b += 1;
        }
        a..b
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot { chars: self.chars.clone(), ids: self.ids.clone(), caret: self.caret, anchor: self.anchor }
    }

    fn checkpoint(&mut self, kind: LastEdit) {
        // Consecutive typing (or deleting) without moving the caret is one undo step.
        if kind != LastEdit::None && kind == self.last {
            return;
        }
        self.undo.push(self.snapshot());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.last = kind;
    }

    /// Replace `range` with `text` (new ids), leaving the caret after the insertion.
    fn splice(&mut self, range: Range<usize>, text: &str) -> Range<usize> {
        let new: Vec<char> = text.chars().collect();
        let ids: Vec<u64> = (0..new.len() as u64).map(|k| self.next_id + k).collect();
        self.next_id += new.len() as u64;
        let start = range.start;
        self.chars.splice(range.clone(), new.iter().copied());
        self.ids.splice(range, ids);
        self.caret = start + new.len();
        self.anchor = None;
        start..start + new.len()
    }

    /// Type or paste `text` over the selection (or at the caret).
    pub fn insert(&mut self, text: &str) {
        let kind = if text.chars().count() == 1 && !text.contains('\n') { LastEdit::Typing } else { LastEdit::None };
        self.checkpoint(kind);
        let sel = self.selection();
        self.splice(sel, text);
        self.marked = None;
    }

    /// Delete the selection, or `range` when nothing is selected. Returns true if anything changed.
    fn delete_or(&mut self, range: Range<usize>) -> bool {
        let sel = self.selection();
        let r = if sel.is_empty() { range } else { sel };
        if r.is_empty() {
            return false;
        }
        self.checkpoint(LastEdit::Deleting);
        self.splice(r, "");
        true
    }

    pub fn backspace(&mut self, word: bool) -> bool {
        let from = if word { self.word_left(self.caret) } else { self.caret.saturating_sub(1) };
        self.delete_or(from..self.caret)
    }

    pub fn delete(&mut self, word: bool) -> bool {
        let to = if word { self.word_right(self.caret) } else { (self.caret + 1).min(self.len()) };
        self.delete_or(self.caret..to)
    }

    pub fn undo(&mut self) -> bool {
        let Some(s) = self.undo.pop() else { return false };
        self.redo.push(self.snapshot());
        self.restore(s);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(s) = self.redo.pop() else { return false };
        self.undo.push(self.snapshot());
        self.restore(s);
        true
    }

    fn restore(&mut self, s: Snapshot) {
        self.chars = s.chars;
        self.ids = s.ids;
        self.caret = s.caret.min(self.chars.len());
        self.anchor = s.anchor;
        self.marked = None;
        self.last = LastEdit::None;
    }

    // ---- IME composition (platform ranges are UTF-16; see to_utf16/from_utf16) ----

    /// Replace `range` (or the marked text, or the selection) with composition `text`, marking it.
    /// `sel_in_text` is the selection inside the new text (char offsets), if the IME provides one.
    pub fn set_marked(&mut self, range: Option<Range<usize>>, text: &str, sel_in_text: Option<Range<usize>>) {
        let r = range.or_else(|| self.marked.clone()).unwrap_or_else(|| self.selection());
        if self.marked.is_none() {
            self.checkpoint(LastEdit::None);
        }
        let placed = self.splice(r, text);
        self.marked = if placed.is_empty() { None } else { Some(placed.clone()) };
        if let Some(s) = sel_in_text {
            let (a, b) = (placed.start + s.start.min(placed.len()), placed.start + s.end.min(placed.len()));
            self.anchor = (a != b).then_some(a);
            self.caret = b;
        }
    }

    /// Commit: replace `range` (or the marked text, or the selection) with final `text`.
    pub fn commit_text(&mut self, range: Option<Range<usize>>, text: &str) {
        // A composition already pushed its undo checkpoint when it started.
        let composing = self.marked.is_some();
        let r = range.or_else(|| self.marked.take()).unwrap_or_else(|| self.selection());
        if !composing {
            self.checkpoint(if text.chars().count() == 1 { LastEdit::Typing } else { LastEdit::None });
        }
        self.splice(r, text);
        self.marked = None;
    }

    pub fn unmark(&mut self) {
        self.marked = None;
    }

    /// Char index → UTF-16 offset.
    pub fn to_utf16(&self, i: usize) -> usize {
        self.chars[..i.min(self.len())].iter().map(|c| c.len_utf16()).sum()
    }

    /// UTF-16 offset → char index (offsets inside a surrogate pair round down).
    pub fn from_utf16(&self, u: usize) -> usize {
        let mut acc = 0;
        for (i, c) in self.chars.iter().enumerate() {
            if acc + c.len_utf16() > u {
                return i;
            }
            acc += c.len_utf16();
        }
        self.len()
    }

    pub fn range_to_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.to_utf16(r.start)..self.to_utf16(r.end)
    }
    pub fn range_from_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.from_utf16(r.start)..self.from_utf16(r.end)
    }
    pub fn len_utf16(&self) -> usize {
        self.to_utf16(self.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_selection_and_ids() {
        let mut d = EditDoc::default();
        for c in ["你", "好", "A"] {
            d.insert(c);
        }
        assert_eq!(d.text(), "你好A");
        assert_eq!(d.ids(), &[0, 1, 2]);
        d.move_to(1, false);
        d.insert("赤");
        assert_eq!(d.text(), "你赤好A");
        assert_eq!(d.ids(), &[0, 3, 1, 2], "existing chars keep their ids");
        d.select_range(1..3);
        assert_eq!(d.selected_text(), "赤好");
        d.insert("写");
        assert_eq!(d.text(), "你写A");
        assert_eq!(d.caret(), 2);
    }

    #[test]
    fn backspace_delete_and_words() {
        let mut d = EditDoc::from_text("hello 你好 world");
        d.move_to(d.len(), false);
        assert!(d.backspace(true));
        assert_eq!(d.text(), "hello 你好 ");
        d.move_to(6, false);
        assert!(d.delete(true)); // CJK char is one word
        assert_eq!(d.text(), "hello 好 ");
        d.move_to(0, false);
        assert!(!d.backspace(false), "nothing before the start");
        assert_eq!(d.word_at(2), 0..5);
    }

    #[test]
    fn undo_coalesces_typing_and_redo_restores() {
        let mut d = EditDoc::default();
        for c in "abc".chars() {
            d.insert(&c.to_string());
        }
        d.insert("\n"); // multi-char/newline insert is its own step
        assert!(d.undo());
        assert_eq!(d.text(), "abc");
        assert!(d.undo());
        assert_eq!(d.text(), "", "typing run undone in one step");
        assert!(!d.undo());
        assert!(d.redo());
        assert_eq!(d.text(), "abc");
        assert!(d.redo());
        assert_eq!(d.text(), "abc\n");
    }

    #[test]
    fn moving_breaks_the_typing_run() {
        let mut d = EditDoc::default();
        d.insert("a");
        d.insert("b");
        d.move_to(0, false);
        d.insert("c");
        assert!(d.undo());
        assert_eq!(d.text(), "ab");
    }

    #[test]
    fn arrows_collapse_selection() {
        let mut d = EditDoc::from_text("abcd");
        d.select_range(1..3);
        d.left(false);
        assert_eq!((d.caret(), d.has_selection()), (1, false));
        d.select_range(1..3);
        d.right(false);
        assert_eq!(d.caret(), 3);
        d.right(true);
        assert_eq!(d.selection(), 3..4);
    }

    #[test]
    fn ime_composition_then_commit() {
        let mut d = EditDoc::from_text("你");
        d.set_marked(None, "ha", Some(2..2));
        assert_eq!(d.text(), "你ha");
        assert_eq!(d.marked(), Some(1..3));
        d.set_marked(None, "hao", Some(3..3));
        assert_eq!(d.text(), "你hao");
        d.commit_text(None, "好");
        assert_eq!(d.text(), "你好");
        assert_eq!(d.marked(), None);
        assert_eq!(d.caret(), 2);
        assert!(d.undo());
        assert_eq!(d.text(), "你", "a composition is one undo step");
    }

    #[test]
    fn utf16_round_trips() {
        let d = EditDoc::from_text("a你😀b");
        assert_eq!(d.len(), 4);
        assert_eq!(d.len_utf16(), 1 + 1 + 2 + 1);
        for i in 0..=d.len() {
            assert_eq!(d.from_utf16(d.to_utf16(i)), i);
        }
        assert_eq!(d.from_utf16(3), 2, "inside a surrogate pair rounds down");
        assert_eq!(d.range_to_utf16(&(2..3)), 2..4);
    }
}
