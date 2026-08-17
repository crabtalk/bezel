//! Editing a [`Doc`].
//!
//! This is the half of a block editor that has nothing to do with gpui: text
//! goes in and out of a [`Text`], marks move with it, and blocks split, merge
//! and indent. Keeping it pure is what makes it testable — the guarantee below
//! is checked over generated edit sequences, not over the handful of cases
//! anyone thinks to write down.
//!
//! **The guarantee is [`crate::serialize`]'s, preserved.** Call
//! [`Doc::normalize`] and the document round-trips: serialize, parse, and
//! nothing moves. An editor that can reach a state its own serializer cannot
//! express is an editor that corrupts the file on save, and no amount of UI
//! polish recovers from that.
//!
//! Normalizing is a *save* step rather than a keystroke step, and deliberately.
//! Markdown cannot hold a space at the end of a line, but stripping one the
//! moment it is typed takes it away mid-word — so the model carries it and
//! sheds it on the way out, which is what every editor that writes markdown
//! does.
//!
//! Marks are **left-sticky**: text typed at the end of a bold run is bold, text
//! typed at its start is not. The caret inherits formatting from the character
//! before it, which is what every editor does and what nobody notices until it
//! is wrong.

use std::ops::Range;

use crate::doc::{Block, BlockKind, Doc, Mark, MarkSpan, Text};

impl Text {
    /// Insert at a byte offset, moving the marks with it.
    pub fn insert(&mut self, at: usize, s: &str) {
        let at = at.min(self.text.len());
        if s.is_empty() {
            return;
        }
        let n = s.len();
        self.text.insert_str(at, s);
        for span in &mut self.marks {
            if at <= span.range.start {
                span.range.start += n;
                span.range.end += n;
            } else if at <= span.range.end {
                // Inside, or exactly at the end — the left-sticky rule.
                span.range.end += n;
            }
        }
        self.normalize_marks();
    }

    /// Remove a byte range, collapsing any mark that covered it.
    pub fn remove(&mut self, range: Range<usize>) {
        let range = self.clamp(range);
        if range.is_empty() {
            return;
        }
        self.text.replace_range(range.clone(), "");
        let shift = |offset: usize| {
            if offset <= range.start {
                offset
            } else if offset >= range.end {
                offset - range.len()
            } else {
                range.start
            }
        };
        for span in &mut self.marks {
            span.range = shift(span.range.start)..shift(span.range.end);
        }
        self.normalize_marks();
    }

    /// Add `mark` over `range`, or take it away if the whole range has it.
    pub fn toggle(&mut self, range: Range<usize>, mark: Mark) {
        let range = self.clamp(range);
        if range.is_empty() {
            return;
        }
        if self.covered_by(&range, &mark) {
            self.marks = std::mem::take(&mut self.marks)
                .into_iter()
                .flat_map(|span| subtract(span, &range, &mark))
                .collect();
        } else {
            self.marks.push(MarkSpan { range, mark });
        }
        self.normalize_marks();
    }

    /// Whether every byte of `range` already carries `mark`.
    pub fn covered_by(&self, range: &Range<usize>, mark: &Mark) -> bool {
        !range.is_empty()
            && self.marks.iter().any(|span| {
                span.mark == *mark && span.range.start <= range.start && span.range.end >= range.end
            })
    }

    /// Cut at `at`, returning the tail. The head keeps this [`Text`].
    pub fn split_off(&mut self, at: usize) -> Text {
        let at = at.min(self.text.len());
        let mut tail = Text {
            text: self.text.split_off(at),
            marks: Vec::new(),
        };
        let mut head = Vec::new();
        for span in std::mem::take(&mut self.marks) {
            if span.range.start < at {
                head.push(MarkSpan {
                    range: span.range.start..span.range.end.min(at),
                    mark: span.mark.clone(),
                });
            }
            if span.range.end > at {
                tail.marks.push(MarkSpan {
                    range: span.range.start.saturating_sub(at)..span.range.end - at,
                    mark: span.mark,
                });
            }
        }
        self.marks = head;
        self.normalize_marks();
        tail.normalize_marks();
        tail
    }

    /// Append `other`, shifting its marks onto the end of this text.
    pub fn append(&mut self, other: Text) {
        let offset = self.text.len();
        self.text.push_str(&other.text);
        self.marks
            .extend(other.marks.into_iter().map(|span| MarkSpan {
                range: span.range.start + offset..span.range.end + offset,
                mark: span.mark,
            }));
        self.normalize_marks();
    }

    fn clamp(&self, range: Range<usize>) -> Range<usize> {
        let start = range.start.min(self.text.len());
        let end = range.end.clamp(start, self.text.len());
        start..end
    }

    /// Drop marks that cover nothing and merge ones that touch.
    ///
    /// Both matter to the round trip rather than to tidiness: an empty bold
    /// span serializes to `****`, which is literal text, and two abutting bold
    /// spans serialize to `**a****b**`, which is not one bold run.
    pub(crate) fn normalize_marks(&mut self) {
        // Emphasis cannot open or close against whitespace — `* t*` is two
        // literal asterisks, not italic — so a mark reaching over a space has
        // no spelling that survives a round trip. Shrinking it to the text it
        // can actually cover is also what a user means when a drag-selection
        // catches the trailing space.
        for ix in 0..self.marks.len() {
            if !matches!(
                self.marks[ix].mark,
                Mark::Bold | Mark::Italic | Mark::Strike | Mark::Code
            ) {
                continue;
            }
            let range = self.marks[ix].range.clone();
            if range.end > self.text.len() {
                continue;
            }
            let slice = &self.text[range.clone()];
            let start = range.start + (slice.len() - slice.trim_start().len());
            let end = (range.end - (slice.len() - slice.trim_end().len())).max(start);
            self.marks[ix].range = start..end;
        }

        let len = self.text.len();
        self.marks.retain(|span| {
            span.range.end <= len && (!span.range.is_empty() || matches!(span.mark, Mark::Image(_)))
        });

        // A code span is atomic: nothing can start or stop inside one. A mark
        // that only half covers it has no spelling, so it grows to take the
        // whole span — which is also what the markdown for it reads back as.
        let code: Vec<Range<usize>> = self
            .marks
            .iter()
            .filter(|span| span.mark == Mark::Code)
            .map(|span| span.range.clone())
            .collect();
        for span in &mut self.marks {
            if span.mark == Mark::Code {
                continue;
            }
            for range in &code {
                let crosses = span.range.start > range.start && span.range.start < range.end
                    || span.range.end > range.start && span.range.end < range.end;
                if crosses {
                    span.range.start = span.range.start.min(range.start);
                    span.range.end = span.range.end.max(range.end);
                }
            }
        }

        // Emphasis nests or it is disjoint; it cannot cross. `**a*b**c*` is
        // not bold-then-italic overlapping, it is a parse error waiting to
        // happen — so when two spans cross, the one that opened first grows to
        // contain the other. Growing rather than clipping keeps every mark the
        // user applied; only its reach changes, and only where markdown left
        // no alternative.
        for _ in 0..self.marks.len().max(1) {
            let mut crossed = false;
            for a in 0..self.marks.len() {
                for b in 0..self.marks.len() {
                    let (first, second) = (&self.marks[a].range, &self.marks[b].range);
                    if second.start > first.start
                        && second.start < first.end
                        && second.end > first.end
                    {
                        let end = second.end;
                        self.marks[a].range.end = end;
                        crossed = true;
                    }
                }
            }
            if !crossed {
                break;
            }
        }

        // Two marks that end at the same offset close as two delimiter runs
        // back to back — `**b**~~`. CommonMark will not let the outer one close
        // there if a letter follows: a run preceded by punctuation has to be
        // followed by whitespace or punctuation to be right-flanking, so
        // `~~a **b**~~c` cannot be written at all. Nudging the outer end past
        // the word separates the two runs and it can.
        for _ in 0..self.marks.len().max(1) {
            let mut nudged = false;
            for a in 0..self.marks.len() {
                let end = self.marks[a].range.end;
                let followed_by_word = self.text[end..]
                    .chars()
                    .next()
                    .is_some_and(char::is_alphanumeric);
                let shared = self.marks.iter().enumerate().any(|(b, other)| {
                    b != a
                        && other.range.end == end
                        && other.range.start > self.marks[a].range.start
                });
                if followed_by_word && shared {
                    let extra = self.text[end..]
                        .find(|c: char| !c.is_alphanumeric())
                        .unwrap_or(self.text.len() - end);
                    self.marks[a].range.end = end + extra;
                    nudged = true;
                }
            }
            if !nudged {
                break;
            }
        }

        let mut ix = 0;
        while ix < self.marks.len() {
            let mut merged = None;
            for other in ix + 1..self.marks.len() {
                let (a, b) = (&self.marks[ix], &self.marks[other]);
                if a.mark == b.mark
                    && a.range.start <= b.range.end
                    && b.range.start <= a.range.end
                    && !matches!(a.mark, Mark::Image(_))
                {
                    merged = Some((
                        other,
                        a.range.start.min(b.range.start),
                        a.range.end.max(b.range.end),
                    ));
                    break;
                }
            }
            match merged {
                Some((other, start, end)) => {
                    self.marks[ix].range = start..end;
                    self.marks.remove(other);
                }
                None => ix += 1,
            }
        }

        // Document order, outermost first — the order a parse produces, so an
        // edited document compares equal to the same document read from disk.
        // The sort is stable, which is what keeps `**_x_**` and `_**x**_`
        // apart: their spans are identical and only their order differs.
        self.marks.sort_by(|a, b| {
            a.range
                .start
                .cmp(&b.range.start)
                .then(b.range.end.cmp(&a.range.end))
        });
    }
}

/// `span` minus `range`, when they share a mark — zero, one or two pieces.
fn subtract(span: MarkSpan, range: &Range<usize>, mark: &Mark) -> Vec<MarkSpan> {
    if span.mark != *mark || span.range.end <= range.start || span.range.start >= range.end {
        return vec![span];
    }
    let mut out = Vec::new();
    if span.range.start < range.start {
        out.push(MarkSpan {
            range: span.range.start..range.start,
            mark: span.mark.clone(),
        });
    }
    if span.range.end > range.end {
        out.push(MarkSpan {
            range: range.end..span.range.end,
            mark: span.mark,
        });
    }
    out
}

impl Doc {
    /// Split block `ix` at byte offset `at`, returning the new block's index.
    ///
    /// The tail keeps the block's kind so Enter in a list makes another item —
    /// except for a heading, where the body that follows a title is body text.
    pub fn split(&mut self, ix: usize, at: usize) -> usize {
        if ix >= self.blocks.len() {
            return ix;
        }
        let indent = self.blocks[ix].indent;
        let tail = match &mut self.blocks[ix].kind {
            BlockKind::Paragraph(text)
            | BlockKind::Heading { text, .. }
            | BlockKind::Bullet(text)
            | BlockKind::Ordered { text, .. }
            | BlockKind::Task { text, .. }
            | BlockKind::Quote(text) => text.split_off(at),
            // Nothing to cut — Enter after an atomic block opens a paragraph.
            _ => Text::default(),
        };
        let kind = match &self.blocks[ix].kind {
            BlockKind::Bullet(_) => BlockKind::Bullet(tail),
            BlockKind::Ordered { .. } => BlockKind::Ordered {
                number: 1,
                text: tail,
            },
            BlockKind::Task { .. } => BlockKind::Task {
                checked: false,
                text: tail,
            },
            BlockKind::Quote(_) => BlockKind::Quote(tail),
            // A heading titles what follows it; what follows is body text.
            _ => BlockKind::Paragraph(tail),
        };
        self.blocks.insert(ix + 1, Block::at(kind, indent));
        self.repair();
        ix + 1
    }

    /// Backspace at the start of block `ix`.
    ///
    /// Notion's chain, in order: an indented block outdents, a list item
    /// becomes a paragraph, and only a plain block at the left margin merges
    /// into the one above it. Returns the caret's new byte offset in whichever
    /// block now holds it, and `None` when nothing moved.
    pub fn merge_back(&mut self, ix: usize) -> Option<usize> {
        let indent = self.blocks.get(ix)?.indent;
        if indent > 0 {
            self.outdent(ix);
            return Some(0);
        }
        if is_marker(&self.blocks[ix].kind) {
            let text = self.blocks[ix].text().cloned().unwrap_or_default();
            self.blocks[ix].kind = BlockKind::Paragraph(text);
            self.repair();
            return Some(0);
        }
        if ix == 0 {
            return None;
        }
        // Only two blocks that both hold text can become one.
        let tail = self.blocks[ix].text()?.clone();
        let caret = self.blocks[ix - 1].text()?.text.len();
        match &mut self.blocks[ix - 1].kind {
            BlockKind::Paragraph(head)
            | BlockKind::Heading { text: head, .. }
            | BlockKind::Bullet(head)
            | BlockKind::Ordered { text: head, .. }
            | BlockKind::Task { text: head, .. }
            | BlockKind::Quote(head) => head.append(tail),
            _ => return None,
        }
        self.blocks.remove(ix);
        self.repair();
        Some(caret)
    }

    /// Apply an edit to block `ix`'s text, then put the block back in order.
    ///
    /// The editor should reach text through here rather than mutating a block
    /// directly: a heading that acquires a newline has no ATX spelling, and
    /// nothing else is positioned to notice.
    pub fn edit_text(&mut self, ix: usize, edit: impl FnOnce(&mut Text)) {
        let Some(block) = self.blocks.get_mut(ix) else {
            return;
        };
        let heading = matches!(block.kind, BlockKind::Heading { .. });
        let text = match &mut block.kind {
            BlockKind::Paragraph(text)
            | BlockKind::Heading { text, .. }
            | BlockKind::Bullet(text)
            | BlockKind::Ordered { text, .. }
            | BlockKind::Task { text, .. }
            | BlockKind::Quote(text) => text,
            _ => return,
        };
        edit(text);
        if heading {
            crate::parse::collapse_to_one_line(text);
        }
    }

    /// Put the document into the form markdown can hold — the save step.
    ///
    /// Drops the whitespace markdown discards anyway (leading and trailing on
    /// every line, blank lines at a block's edges), flattens the blocks whose
    /// output is one line, and renumbers ordered runs. After this,
    /// `parse(serialize(doc)) == doc`.
    pub fn normalize(&mut self) {
        for block in &mut self.blocks {
            let one_line = matches!(block.kind, BlockKind::Heading { .. });
            match &mut block.kind {
                BlockKind::Paragraph(text)
                | BlockKind::Heading { text, .. }
                | BlockKind::Bullet(text)
                | BlockKind::Ordered { text, .. }
                | BlockKind::Task { text, .. }
                | BlockKind::Quote(text) => {
                    *text = crate::parse::normalize(&text.text, &text.marks);
                    text.normalize_marks();
                    if one_line {
                        crate::parse::collapse_to_one_line(text);
                    }
                }
                BlockKind::Table { header, rows, .. } => {
                    for cell in header.iter_mut().chain(rows.iter_mut().flatten()) {
                        *cell = crate::parse::normalize(&cell.text, &cell.marks);
                        cell.normalize_marks();
                        crate::parse::collapse_to_one_line(cell);
                    }
                }
                BlockKind::Code { .. } | BlockKind::Image { .. } | BlockKind::Rule => {}
            }
        }
        // A blank paragraph is the empty line an editor leaves behind, and
        // markdown has no way to write one down — blank lines there separate
        // blocks rather than being one. An empty heading or list item is
        // different: `# ` and `- ` are both real, so those stay.
        self.blocks.retain(|block| {
            !matches!(
                &block.kind,
                BlockKind::Paragraph(text) | BlockKind::Quote(text) if text.is_empty()
            )
        });
        self.repair();

        // The rules above keep every ordinary edit lossless. They cannot be
        // complete, and no serializer fix would make them so: whether a mark
        // boundary can be written depends on CommonMark's flanking rules, and
        // some marks have no spelling at all. Bold ending on a `~` with a letter
        // after it is one — a closing delimiter preceded by punctuation and
        // followed by a letter is not right-flanking, so `Tit**l\~\~**e` does
        // not close. That is a limit of the format, not a bug in the writer.
        //
        // So the last word goes to markdown: adopt the document it can hold.
        //
        // This is exact rather than approximate. Anything [`crate::parse`]
        // returns is a fixed point of the round trip — that is the guarantee the
        // parser is tested for — so writing this document out and reading it
        // back yields one by construction. Marks with no spelling are dropped
        // here, in front of the reader, rather than silently at save time.
        //
        // The cheaper rules above still earn their place: they are what keeps
        // the ordinary edit lossless, so this step has nothing left to take.
        *self = crate::parse(&crate::serialize(self));
    }

    /// Tab. A block can go one level deeper than the one above it, and its
    /// children come with it.
    pub fn indent(&mut self, ix: usize) -> bool {
        let Some(block) = self.blocks.get(ix) else {
            return false;
        };
        if block.indent >= self.ceiling(ix) {
            return false;
        }
        self.shift_subtree(ix, 1);
        self.repair();
        true
    }

    /// How deep block `ix` is allowed to sit.
    ///
    /// Markdown expresses nesting through list items and nothing else, so a
    /// block may only go deeper than the one above it when that one is a
    /// marker. Indenting a paragraph under a *heading* would serialize to four
    /// leading spaces, which reads back as an indented code block.
    fn ceiling(&self, ix: usize) -> u8 {
        match ix.checked_sub(1).map(|previous| &self.blocks[previous]) {
            None => 0,
            Some(previous) if is_marker(&previous.kind) => previous.indent + 1,
            Some(previous) => previous.indent,
        }
    }

    /// Clamp every indent to what the document can actually express, then make
    /// ordered runs consecutive. Cheap, total, and called after anything
    /// structural — a local rule is not enough, because outdenting one block
    /// can leave the block *after* it stranded a level too deep.
    ///
    /// Public because an editor that changes a block's *kind* has to restore
    /// the invariant too, and only this knows what it is.
    pub fn repair(&mut self) {
        for ix in 0..self.blocks.len() {
            let ceiling = self.ceiling(ix);
            self.blocks[ix].indent = self.blocks[ix].indent.min(ceiling);
        }
        self.renumber();
    }

    /// Shift-Tab, children included.
    pub fn outdent(&mut self, ix: usize) -> bool {
        if self.blocks.get(ix).is_none_or(|block| block.indent == 0) {
            return false;
        }
        self.shift_subtree(ix, -1);
        self.repair();
        true
    }

    /// Move a block and everything nested under it. Children have to travel
    /// with the parent or the document invariant breaks the moment a level
    /// disappears from under them.
    fn shift_subtree(&mut self, ix: usize, by: i8) {
        let base = self.blocks[ix].indent;
        let mut end = ix + 1;
        while self
            .blocks
            .get(end)
            .is_some_and(|block| block.indent > base)
        {
            end += 1;
        }
        for block in &mut self.blocks[ix..end] {
            block.indent = block.indent.saturating_add_signed(by);
        }
    }
}

fn is_marker(kind: &BlockKind) -> bool {
    matches!(
        kind,
        BlockKind::Bullet(_) | BlockKind::Ordered { .. } | BlockKind::Task { .. }
    )
}

/// A markdown prefix typed at the start of a block, and what it turns it into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    Heading(u8),
    Bullet,
    Ordered,
    Task(bool),
    Quote,
    Code,
    Rule,
}

impl Shortcut {
    /// The block this shortcut makes, carrying whatever text was left over.
    pub fn apply(self, text: Text) -> BlockKind {
        match self {
            Self::Heading(level) => BlockKind::Heading { level, text },
            Self::Bullet => BlockKind::Bullet(text),
            Self::Ordered => BlockKind::Ordered { number: 1, text },
            Self::Task(checked) => BlockKind::Task { checked, text },
            Self::Quote => BlockKind::Quote(text),
            Self::Code => BlockKind::Code {
                language: None,
                code: text.text,
            },
            Self::Rule => BlockKind::Rule,
        }
    }
}

/// Match a markdown prefix at the start of a block, returning it and how many
/// bytes it occupied.
///
/// This is the input side of the same vocabulary [`crate::parse`] reads: typing
/// `## ` makes a heading because pasting `## ` would have. Order matters — a
/// task marker is a bullet with more on the end.
pub fn shortcut(text: &str) -> Option<(Shortcut, usize)> {
    const PREFIXES: &[(&str, Shortcut)] = &[
        ("- [ ] ", Shortcut::Task(false)),
        ("- [x] ", Shortcut::Task(true)),
        ("###### ", Shortcut::Heading(6)),
        ("##### ", Shortcut::Heading(5)),
        ("#### ", Shortcut::Heading(4)),
        ("### ", Shortcut::Heading(3)),
        ("## ", Shortcut::Heading(2)),
        ("# ", Shortcut::Heading(1)),
        ("- ", Shortcut::Bullet),
        ("* ", Shortcut::Bullet),
        ("+ ", Shortcut::Bullet),
        ("1. ", Shortcut::Ordered),
        ("> ", Shortcut::Quote),
        ("```", Shortcut::Code),
        ("---", Shortcut::Rule),
    ];
    PREFIXES
        .iter()
        .find(|(prefix, _)| text.starts_with(prefix))
        .map(|(prefix, shortcut)| (*shortcut, prefix.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse::parse, serialize::serialize};

    fn text_of(doc: &Doc, ix: usize) -> &Text {
        doc.blocks[ix].text().expect("block holds text")
    }

    #[test]
    fn typing_at_the_end_of_a_mark_joins_it() {
        let mut text = parse("**ab**").blocks[0].text().unwrap().clone();
        text.insert(2, "c");
        assert_eq!(text.text, "abc");
        assert_eq!(text.marks[0].range, 0..3, "left-sticky: the tail is bold");
    }

    #[test]
    fn typing_at_the_start_of_a_mark_stays_outside_it() {
        let mut text = parse("**ab**").blocks[0].text().unwrap().clone();
        text.insert(0, "x");
        assert_eq!(text.text, "xab");
        assert_eq!(text.marks[0].range, 1..3);
    }

    #[test]
    fn removing_text_collapses_the_marks_over_it() {
        let mut text = parse("a**bc**d").blocks[0].text().unwrap().clone();
        text.remove(1..3);
        assert_eq!(text.text, "ad");
        assert!(
            text.marks
                .iter()
                .all(|span| span.range.end <= text.text.len())
        );
    }

    #[test]
    fn toggling_twice_is_a_no_op() {
        let mut text = Text::plain("hello");
        let before = text.clone();
        text.toggle(0..5, Mark::Bold);
        assert!(text.covered_by(&(0..5), &Mark::Bold));
        text.toggle(0..5, Mark::Bold);
        assert_eq!(text, before);
    }

    #[test]
    fn untoggling_the_middle_leaves_the_ends() {
        let mut text = Text::plain("abcde");
        text.toggle(0..5, Mark::Bold);
        text.toggle(1..4, Mark::Bold);
        let mut ranges: Vec<_> = text.marks.iter().map(|span| span.range.clone()).collect();
        ranges.sort_by_key(|range| range.start);
        assert_eq!(ranges, vec![0..1, 4..5]);
    }

    #[test]
    fn abutting_marks_merge_so_they_serialize_as_one_run() {
        let mut text = Text::plain("abcd");
        text.toggle(0..2, Mark::Bold);
        text.toggle(2..4, Mark::Bold);
        assert_eq!(text.marks.len(), 1, "`**ab****cd**` is not one bold run");
        assert_eq!(text.marks[0].range, 0..4);
    }

    #[test]
    fn enter_in_a_list_makes_another_item() {
        let mut doc = parse("- alpha");
        let new = doc.split(0, 3);
        assert_eq!(new, 1);
        assert!(matches!(doc.blocks[1].kind, BlockKind::Bullet(_)));
        assert_eq!(text_of(&doc, 0).text, "alp");
        assert_eq!(text_of(&doc, 1).text, "ha");
    }

    #[test]
    fn enter_after_a_heading_gives_body_text() {
        let mut doc = parse("# Title");
        doc.split(0, 5);
        assert!(matches!(doc.blocks[1].kind, BlockKind::Paragraph(_)));
    }

    #[test]
    fn backspace_walks_out_before_it_merges() {
        // Indented bullet: outdent, then unmarker, then merge.
        let mut doc = parse("- a\n    - b");
        assert_eq!(doc.blocks[1].indent, 1);

        doc.merge_back(1);
        assert_eq!(doc.blocks[1].indent, 0, "first press outdents");

        doc.merge_back(1);
        assert!(
            matches!(doc.blocks[1].kind, BlockKind::Paragraph(_)),
            "second press drops the marker"
        );

        let caret = doc.merge_back(1);
        assert_eq!(caret, Some(1), "third press merges after \"a\"");
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(text_of(&doc, 0).text, "ab");
    }

    #[test]
    fn indenting_carries_the_children() {
        let mut doc = parse("- a\n- b\n    - c");
        assert!(doc.indent(1));
        assert_eq!(doc.blocks[1].indent, 1);
        assert_eq!(doc.blocks[2].indent, 2, "the child came along");
    }

    #[test]
    fn a_block_cannot_indent_more_than_one_past_the_one_above() {
        let mut doc = parse("- a\n- b");
        assert!(doc.indent(1));
        assert!(
            !doc.indent(1),
            "two levels in one press would break the invariant"
        );
        assert!(!doc.indent(0), "the first block has nothing to nest under");
    }

    #[test]
    fn shortcuts_cover_the_slash_menu_vocabulary() {
        assert_eq!(shortcut("# x"), Some((Shortcut::Heading(1), 2)));
        assert_eq!(shortcut("### x"), Some((Shortcut::Heading(3), 4)));
        assert_eq!(shortcut("- [x] x"), Some((Shortcut::Task(true), 6)));
        assert_eq!(shortcut("- [ ] x"), Some((Shortcut::Task(false), 6)));
        assert_eq!(shortcut("- x"), Some((Shortcut::Bullet, 2)));
        assert_eq!(shortcut("1. x"), Some((Shortcut::Ordered, 3)));
        assert_eq!(shortcut("> x"), Some((Shortcut::Quote, 2)));
        assert_eq!(shortcut("plain"), None);
        // A task marker must win over the bullet it starts with.
        assert_ne!(shortcut("- [ ] x").map(|hit| hit.0), Some(Shortcut::Bullet));
    }

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 33) as usize
        }
    }

    /// The property that matters: no sequence of edits can reach a document
    /// the serializer cannot express. If this fails, the editor corrupts the
    /// file on save.
    #[test]
    fn no_edit_sequence_escapes_the_round_trip() {
        const SEEDS: &[&str] = &[
            "# Title\n\nBody **bold** text",
            "- a\n- b\n    - c",
            "1. one\n2. two",
            "- [ ] task\n- [x] done",
            "> quoted\n\npara `code` tail",
            "para with [link](u) and ![img](i)",
        ];
        const WORDS: &[&str] = &["x", " ", "\n", "a b", "**", "#", "- ", "`", "|", "~~"];
        const MARKS: &[Mark] = &[Mark::Bold, Mark::Italic, Mark::Strike, Mark::Code];

        let mut rng = Rng(0xed17);
        for seed in SEEDS {
            for case in 0..20_000 {
                let mut doc = parse(seed);

                for _ in 0..6 {
                    if doc.blocks.is_empty() {
                        break;
                    }
                    let ix = rng.next() % doc.blocks.len();
                    let len = doc.blocks[ix].text().map_or(0, |text| text.text.len());
                    let a = if len == 0 { 0 } else { rng.next() % (len + 1) };
                    let b = if len == 0 { 0 } else { rng.next() % (len + 1) };

                    match rng.next() % 7 {
                        0 => {
                            let word = WORDS[rng.next() % WORDS.len()];
                            doc.edit_text(ix, |text| text.insert(a, word));
                        }
                        1 => {
                            doc.edit_text(ix, |text| text.remove(a.min(b)..a.max(b)));
                        }
                        2 => {
                            let mark = MARKS[rng.next() % MARKS.len()].clone();
                            doc.edit_text(ix, |text| text.toggle(a.min(b)..a.max(b), mark));
                        }
                        3 => {
                            doc.split(ix, a);
                        }
                        4 => {
                            doc.merge_back(ix);
                        }
                        5 => {
                            doc.indent(ix);
                        }
                        _ => {
                            doc.outdent(ix);
                        }
                    }

                    // Offsets must stay inside the text they index.
                    for block in &doc.blocks {
                        if let Some(text) = block.text() {
                            for span in &text.marks {
                                assert!(
                                    span.range.end <= text.text.len()
                                        && text.text.is_char_boundary(span.range.start)
                                        && text.text.is_char_boundary(span.range.end),
                                    "mark escaped its text: {span:?} in {:?}",
                                    text.text
                                );
                            }
                        }
                    }

                    // And the document invariant the serializer leans on.
                    for ix in 0..doc.blocks.len() {
                        assert!(
                            doc.blocks[ix].indent <= doc.ceiling(ix),
                            "block {ix} is deeper than anything above it can hold"
                        );
                    }
                }

                doc.normalize();
                let text = serialize(&doc);
                assert_eq!(
                    parse(&text),
                    doc,
                    "seed {seed:?} case {case}: edits left a document that does not \
                     survive its own serializer\n--- serialized ---\n{text:?}\n"
                );
            }
        }
    }
}

/// A caret: which block, and how far into its text.
///
/// Byte offsets, like the marks — and like them, always on a character
/// boundary. It lives here rather than in the editor crate because a position
/// in a document is part of the document model: the pure half can move it,
/// which is what makes the motion testable without a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    pub block: usize,
    pub offset: usize,
}

impl Cursor {
    pub fn new(block: usize, offset: usize) -> Self {
        Self { block, offset }
    }

    /// How much text block `ix` holds, or `None` if it holds none at all —
    /// code, images, rules and tables are atomic to a caret.
    fn len_of(doc: &Doc, ix: usize) -> Option<usize> {
        doc.blocks.get(ix)?.text().map(|text| text.text.len())
    }

    /// Pull the caret back onto a real position: a block that exists, an offset
    /// inside it, and a character boundary.
    pub fn clamp(self, doc: &Doc) -> Self {
        if doc.blocks.is_empty() {
            return Self::default();
        }
        let block = self.block.min(doc.blocks.len() - 1);
        let Some(len) = Self::len_of(doc, block) else {
            return Self { block, offset: 0 };
        };
        let mut offset = self.offset.min(len);
        if let Some(text) = doc.blocks[block].text() {
            while offset > 0 && !text.text.is_char_boundary(offset) {
                offset -= 1;
            }
        }
        Self { block, offset }
    }

    /// The nearest block at or after `from` that a caret can sit in.
    fn next_editable(doc: &Doc, from: usize) -> Option<usize> {
        (from..doc.blocks.len()).find(|ix| Self::len_of(doc, *ix).is_some())
    }

    /// The nearest block at or before `from` that a caret can sit in.
    fn previous_editable(doc: &Doc, from: usize) -> Option<usize> {
        (0..=from.min(doc.blocks.len().saturating_sub(1)))
            .rev()
            .find(|ix| Self::len_of(doc, *ix).is_some())
    }

    /// One character left, stepping into the block above at the start of a line.
    pub fn left(self, doc: &Doc) -> Self {
        let here = self.clamp(doc);
        if here.offset > 0 {
            let text = &doc.blocks[here.block].text().expect("clamped").text;
            let mut offset = here.offset - 1;
            while offset > 0 && !text.is_char_boundary(offset) {
                offset -= 1;
            }
            return Self { offset, ..here };
        }
        match here
            .block
            .checked_sub(1)
            .and_then(|ix| Self::previous_editable(doc, ix))
        {
            Some(block) => Self {
                block,
                offset: Self::len_of(doc, block).unwrap_or(0),
            },
            None => here,
        }
    }

    /// One character right, stepping into the block below at the end of a line.
    pub fn right(self, doc: &Doc) -> Self {
        let here = self.clamp(doc);
        let len = Self::len_of(doc, here.block).unwrap_or(0);
        if here.offset < len {
            let text = &doc.blocks[here.block].text().expect("clamped").text;
            let mut offset = here.offset + 1;
            while offset < len && !text.is_char_boundary(offset) {
                offset += 1;
            }
            return Self { offset, ..here };
        }
        match Self::next_editable(doc, here.block + 1) {
            Some(block) => Self { block, offset: 0 },
            None => here,
        }
    }

    /// The block above, keeping the offset where it fits.
    pub fn up(self, doc: &Doc) -> Self {
        let here = self.clamp(doc);
        match here
            .block
            .checked_sub(1)
            .and_then(|ix| Self::previous_editable(doc, ix))
        {
            Some(block) => Self {
                block,
                offset: here.offset,
            }
            .clamp(doc),
            None => Self { offset: 0, ..here },
        }
    }

    /// The block below, keeping the offset where it fits.
    pub fn down(self, doc: &Doc) -> Self {
        let here = self.clamp(doc);
        match Self::next_editable(doc, here.block + 1) {
            Some(block) => Self {
                block,
                offset: here.offset,
            }
            .clamp(doc),
            None => Self {
                offset: Self::len_of(doc, here.block).unwrap_or(0),
                ..here
            },
        }
    }

    pub fn home(self) -> Self {
        Self { offset: 0, ..self }
    }

    pub fn end(self, doc: &Doc) -> Self {
        let here = self.clamp(doc);
        Self {
            offset: Self::len_of(doc, here.block).unwrap_or(0),
            ..here
        }
    }
}
