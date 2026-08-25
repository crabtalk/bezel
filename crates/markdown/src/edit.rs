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

use crate::{
    doc::{Block, BlockKind, Doc, Mark, MarkSpan, Part, Text},
    select::{Cursor, Selection},
};

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
                    && !matches!(a.mark, Mark::Image(_) | Mark::Mention { .. })
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
        // Nothing to cut for a block with no body — Enter after an atomic
        // block opens a paragraph.
        let tail = self.blocks[ix]
            .text_at_mut(Part::Body)
            .map(|text| text.split_off(at))
            .unwrap_or_default();
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

    /// Backspace at the start of a block.
    ///
    /// Notion's chain, in order: an indented block outdents, an image with
    /// nothing written under it goes, a block wearing syntax around its text
    /// gives the syntax up, and only a plain block at the left margin merges
    /// into the one above it. When that one holds no body there is nothing to
    /// merge into, so the caret steps into a fence or a table, and a rule —
    /// which no caret can enter, and so no other key can remove — goes.
    /// Returns where the caret landed, and `None` when nothing moved.
    ///
    /// A table cell is not a position that can swallow its neighbour, so
    /// backspace at the start of one does nothing rather than eating the table.
    pub fn merge_back(&mut self, at: Cursor) -> Option<Cursor> {
        if matches!(at.part, Part::Cell { .. }) {
            return None;
        }
        let block = self.blocks.get(at.block)?;
        if block.indent > 0 {
            self.outdent(at.block);
            return Some(Cursor::new(at.block, at.part, 0));
        }
        // A caption is the only handle a caret has on an image, so with the
        // caption empty there is nothing left to take but the picture.
        if at.part == Part::Caption && block.text_at(Part::Caption)?.is_empty() {
            let previous = at.block.checked_sub(1);
            self.blocks.remove(at.block);
            self.repair();
            let Some(previous) = previous else {
                return Some(Cursor::default().clamp(self));
            };
            let part = self.blocks[previous]
                .parts()
                .last()
                .copied()
                .unwrap_or_default();
            return Some(Cursor::new(previous, part, 0).end(self));
        }
        // Every prefix [`shortcut`] reads is chrome around text; the first
        // backspace takes the chrome and leaves the text where it was, so what
        // can be typed in can be typed out.
        let unwrapped = match &block.kind {
            kind if is_marker(kind) => block.text_at(Part::Body).cloned(),
            BlockKind::Heading { text, .. } | BlockKind::Quote(text) => Some(text.clone()),
            BlockKind::Code { code, .. } => Some(code.clone()),
            _ => None,
        };
        if let Some(text) = unwrapped {
            self.blocks[at.block].kind = BlockKind::Paragraph(text);
            self.repair();
            return Some(Cursor::new(at.block, Part::Body, 0));
        }
        if at.block == 0 {
            return None;
        }
        let tail = self.blocks[at.block].text_at(Part::Body)?.clone();
        let previous = at.block - 1;
        match self.blocks[previous].parts().last().copied() {
            // Only two blocks that both hold a body can become one.
            Some(Part::Body) => {
                let head = self.blocks[previous].text_at_mut(Part::Body)?;
                let caret = head.text.len();
                head.append(tail);
                self.blocks.remove(at.block);
                self.repair();
                Some(Cursor::new(previous, Part::Body, caret))
            }
            Some(part) => {
                let end = self.blocks[previous]
                    .text_at(part)
                    .map_or(0, |text| text.text.len());
                Some(Cursor::new(previous, part, end))
            }
            None => {
                self.blocks.remove(previous);
                self.repair();
                Some(Cursor::new(previous, at.part, at.offset))
            }
        }
    }

    /// Apply an edit to the text at `at`, then put the block back in order.
    ///
    /// The editor should reach text through here rather than mutating a block
    /// directly: a heading or a table cell that acquires a newline has no
    /// spelling, and nothing else is positioned to notice.
    pub fn edit_at(&mut self, at: Cursor, edit: impl FnOnce(&mut Text)) {
        let Some(block) = self.blocks.get_mut(at.block) else {
            return;
        };
        let one_line = matches!(block.kind, BlockKind::Heading { .. })
            || matches!(at.part, Part::Cell { .. } | Part::Caption);
        let Some(text) = block.text_at_mut(at.part) else {
            return;
        };
        edit(text);
        if one_line {
            crate::parse::collapse_to_one_line(text);
        }
    }

    /// The blocks nested under `ix`, `ix` included — what a move, a duplicate
    /// or a drag carries with it.
    ///
    /// A flat list makes this a scan for the next block that is not deeper,
    /// which is the whole argument for the flat list.
    pub fn subtree(&self, ix: usize) -> Range<usize> {
        let Some(base) = self.blocks.get(ix).map(|block| block.indent) else {
            return ix..ix;
        };
        let mut end = ix + 1;
        while self
            .blocks
            .get(end)
            .is_some_and(|block| block.indent > base)
        {
            end += 1;
        }
        ix..end
    }

    /// Move a block and its children to sit before or after their neighbour.
    ///
    /// `delta` counts *siblings*, not rows: moving down past a bullet with
    /// three children clears all four, or a block would land inside the run it
    /// was trying to step over.
    pub fn move_block(&mut self, ix: usize, delta: isize) -> Option<usize> {
        let span = self.subtree(ix);
        if span.is_empty() {
            return None;
        }
        let to = match delta {
            ..0 => {
                // The start of whichever subtree ends where this one begins.
                (0..span.start)
                    .rev()
                    .find(|&above| self.subtree(above).end == span.start)?
            }
            0.. => {
                let next = self.subtree(span.end);
                if next.is_empty() {
                    return None;
                }
                // Landing after the neighbour means landing where it ends,
                // less the hole this subtree leaves behind.
                next.end - span.len()
            }
        };
        let moved: Vec<Block> = self.blocks.drain(span.clone()).collect();
        self.blocks.splice(to..to, moved);
        self.repair();
        Some(to)
    }

    /// Copy a block and its children in below themselves.
    pub fn duplicate(&mut self, ix: usize) -> Option<usize> {
        let span = self.subtree(ix);
        if span.is_empty() {
            return None;
        }
        let copy: Vec<Block> = self.blocks[span.clone()].to_vec();
        self.blocks.splice(span.end..span.end, copy);
        self.repair();
        Some(span.end)
    }

    /// Delete a block and its children.
    pub fn remove_block(&mut self, ix: usize) {
        let span = self.subtree(ix);
        if span.is_empty() {
            return;
        }
        self.blocks.drain(span);
        self.repair();
    }

    /// Turn block `ix` into `kind`, carrying its text across and keeping its
    /// indent.
    ///
    /// The one operation a typed prefix, the slash menu and the block menu all
    /// perform, so none of them reaches into a block's kind on its own.
    pub fn set_kind(&mut self, ix: usize, kind: BlockKind) {
        let Some(block) = self.blocks.get_mut(ix) else {
            return;
        };
        let text = match &block.kind {
            // A bookmark's text is the link it shows, so turning one back into
            // prose hands the URL over instead of an empty block.
            BlockKind::Bookmark { url, .. } => Text::link(url),
            BlockKind::Image { alt, .. } => alt.clone(),
            _ => block.text_at(Part::Body).cloned().unwrap_or_default(),
        };
        block.kind = kind;
        match block.text_at_mut(Part::Body) {
            Some(body) => *body = text,
            // The two kinds whose text is not a body. Code is also the one the
            // marks cannot come with.
            None => match &mut block.kind {
                BlockKind::Code { code, .. } => *code = Text::plain(text.text),
                BlockKind::Image { alt, .. } => *alt = text,
                _ => {}
            },
        }
        self.repair();
    }

    /// The tag on a fenced block — what the label shows, what the highlighter
    /// reads, and what the info string carries. Not [`Doc::set_kind`]'s job:
    /// that carries a *body* across, and a fence has none to give back.
    pub fn set_language(&mut self, ix: usize, language: Option<String>) {
        if let Some(BlockKind::Code { language: tag, .. }) =
            self.blocks.get_mut(ix).map(|block| &mut block.kind)
        {
            *tag = language;
        }
    }

    /// Turn what a selection covers into one code block, leaving whatever it
    /// did not cover as blocks of its own.
    ///
    /// The fence is what markdown has for code over more than one line. An
    /// inline span is not: no CommonMark spelling puts a line break inside
    /// backticks, so one written that way comes back as a space.
    ///
    /// Marks are dropped on the way in, the way [`Doc::set_kind`] drops them
    /// when it turns a block into a fence — code is literal to its closing
    /// fence, and nothing in it is markup.
    pub fn fence(&mut self, selection: Selection) -> Cursor {
        let lines: Vec<String> = self
            .spans(selection)
            .iter()
            .filter(|(at, _)| at.part == Part::Body)
            .filter_map(|(at, range)| {
                let text = self.blocks[at.block].text_at(at.part)?;
                text.text.get(range.clone()).map(str::to_string)
            })
            .collect();
        if lines.is_empty() {
            return selection.head.clamp(self);
        }
        let code = Text::plain(lines.join("\n"));

        // Cutting the selection leaves the head and the tail it did not cover
        // joined in one block, with the caret at the seam between them — which
        // is where the fence goes.
        let at = self.replace(selection, Text::default());
        let tail = self.split(at.block, at.offset);
        let indent = self.blocks[at.block].indent;
        self.blocks.insert(
            tail,
            Block::at(
                BlockKind::Code {
                    language: None,
                    code,
                },
                indent,
            ),
        );
        // A selection that covered whole blocks leaves nothing on either side,
        // and an empty paragraph is not what "turn this into code" asked for.
        let empty = |block: &Block| {
            block
                .text_at(Part::Body)
                .is_some_and(|text| text.text.is_empty())
        };
        if self.blocks.get(tail + 1).is_some_and(empty) {
            self.blocks.remove(tail + 1);
        }
        let mut fence = tail;
        if empty(&self.blocks[at.block]) {
            self.blocks.remove(at.block);
            fence -= 1;
        }
        self.repair();
        Cursor::new(fence, Part::Code, 0).clamp(self)
    }

    /// The way back out of a fence: every line becomes a paragraph. `None` when
    /// the selection is not all code, which is what makes this the other half
    /// of a toggle rather than an operation of its own.
    pub fn unfence(&mut self, selection: Selection) -> Option<Cursor> {
        let (start, end) = selection.clamp(self).ordered();
        let blocks = start.block..=end.block;
        if !blocks
            .clone()
            .all(|ix| matches!(self.blocks[ix].kind, BlockKind::Code { .. }))
        {
            return None;
        }
        for ix in blocks.rev() {
            let BlockKind::Code { code, .. } = &self.blocks[ix].kind else {
                continue;
            };
            let indent = self.blocks[ix].indent;
            let paragraphs: Vec<Block> = code
                .text
                .split('\n')
                .map(|line| Block::at(BlockKind::Paragraph(Text::plain(line)), indent))
                .collect();
            self.blocks.splice(ix..=ix, paragraphs);
        }
        self.repair();
        Some(Cursor::new(start.block, Part::Body, 0).clamp(self))
    }

    /// Every text a selection touches, with the slice of it covered.
    ///
    /// One selection can reach across paragraphs and table cells, and a mark
    /// applies to each of them separately — marks live inside a [`Text`] and
    /// have no way to span two.
    pub fn spans(&self, selection: Selection) -> Vec<(Cursor, Range<usize>)> {
        let (start, end) = selection.clamp(self).ordered();
        let (first, last) = (
            Cursor::new(start.block, start.part, 0),
            Cursor::new(end.block, end.part, 0),
        );
        let mut out = Vec::new();
        for block in start.block..=end.block.min(self.blocks.len().saturating_sub(1)) {
            for part in self.blocks[block].parts() {
                let here = Cursor::new(block, part, 0);
                if here < first || here > last {
                    continue;
                }
                let len = here.len_in(self).unwrap_or(0);
                let from = if here == first { start.offset } else { 0 };
                let to = if here == last { end.offset } else { len };
                if from < to.min(len) {
                    out.push((here, from..to.min(len)));
                }
            }
        }
        out
    }

    /// Add `mark` over a selection, or take it away if every part of the
    /// selection already carries it.
    ///
    /// The decision is made across the whole selection before anything moves:
    /// dragging over a bold word and a plain one and pressing cmd-B should bold
    /// the rest rather than unbolding the half that was already there.
    pub fn toggle_mark(&mut self, selection: Selection, mark: Mark) {
        let spans = self.spans(selection);
        let remove = self.covered_by(selection, &mark);

        for (at, range) in spans {
            // Code is literal to its closing fence and a caption has no room
            // for markup between its brackets; nothing in either is markup.
            if matches!(at.part, Part::Code | Part::Caption) {
                continue;
            }
            if remove == self.carries(&at, &range, &mark) {
                let mark = mark.clone();
                self.edit_at(at, |text| text.toggle(range, mark));
            }
        }
    }

    /// Whether every part of a selection already carries `mark` — what decides
    /// between adding it and taking it away, and what a toolbar button reads to
    /// know whether it is lit.
    pub fn covered_by(&self, selection: Selection, mark: &Mark) -> bool {
        let spans = self.spans(selection);
        !spans.is_empty()
            && spans.iter().all(|(at, range)| {
                matches!(at.part, Part::Code | Part::Caption) || self.carries(at, range, mark)
            })
    }

    fn carries(&self, at: &Cursor, range: &Range<usize>, mark: &Mark) -> bool {
        self.blocks[at.block]
            .text_at(at.part)
            .is_some_and(|text| text.covered_by(range, mark))
    }

    /// The sub-document a selection covers — what a copy puts on the clipboard.
    ///
    /// A table is atomic here for the same reason it is in [`Doc::replace`]:
    /// half a table has no shape worth keeping, so a selection reaching into
    /// one takes it whole.
    pub fn slice(&self, selection: Selection) -> Doc {
        let (start, end) = selection.clamp(self).ordered();
        let mut out = Doc {
            blocks: self.blocks[start.block..=end.block].to_vec(),
        };
        let last = end.block - start.block;
        // Tail first: trimming the head would move the offsets the tail is in.
        if !matches!(end.part, Part::Cell { .. })
            && let Some(text) = out.blocks[last].text_at_mut(end.part)
        {
            text.split_off(end.offset);
        }
        if !matches!(start.part, Part::Cell { .. })
            && let Some(text) = out.blocks[0].text_at_mut(start.part)
        {
            *text = text.split_off(start.offset);
        }
        // The slice starts at the left margin whatever depth it was cut from.
        out.repair();
        out
    }

    /// Replace a selection with a whole document — the paste path.
    ///
    /// A lone paragraph goes in as inline text, marks and all: pasting a
    /// sentence into a sentence must not make a new block. Anything else
    /// arrives as blocks, and the remainder of the caret's block follows them.
    pub fn splice(&mut self, selection: Selection, other: Doc) -> Cursor {
        let blocks = other.blocks;
        let inline = match blocks.as_slice() {
            [] => Some(Text::default()),
            [block] => match &block.kind {
                BlockKind::Paragraph(text) => Some(text.clone()),
                _ => None,
            },
            _ => None,
        };
        if let Some(text) = inline {
            return self.replace(selection, text);
        }

        let caret = self.replace(selection, Text::default());
        let base = self.blocks[caret.block].indent;
        // Split so what followed the caret follows the paste too. An empty
        // remainder is the blank block a paste at the end would leave behind.
        let tail = self.split(caret.block, caret.offset);
        let empty_tail = self.blocks[tail]
            .text_at(Part::Body)
            .is_some_and(Text::is_empty);

        let mut at = caret.block;
        for block in blocks {
            at += 1;
            self.blocks
                .insert(at, Block::at(block.kind, base.saturating_add(block.indent)));
        }
        if empty_tail {
            self.blocks.remove(at + 1);
        }
        // And the block the caret opened in, if the paste displaced all of it.
        let head_empty = self.blocks[caret.block]
            .text_at(Part::Body)
            .is_some_and(Text::is_empty);
        if head_empty && matches!(self.blocks[caret.block].kind, BlockKind::Paragraph(_)) {
            self.blocks.remove(caret.block);
            at -= 1;
        }
        self.repair();
        Cursor::new(at, Part::Body, 0).end(self).clamp(self)
    }

    /// Replace everything a selection covers with `text`, and say where the
    /// caret lands.
    ///
    /// **The one mutation.** Typing, backspace, delete, cut and paste are all
    /// this call with a different argument, which is why none of them needs to
    /// know whether a selection was empty, spanned two paragraphs, or swallowed
    /// a table on the way past.
    pub fn replace(&mut self, selection: Selection, text: Text) -> Cursor {
        // An empty document has no block to put anything in; editing one opens
        // the paragraph every other path then assumes exists.
        if self.blocks.is_empty() {
            self.blocks
                .push(Block::new(BlockKind::Paragraph(Text::default())));
        }
        let (start, end) = selection.clamp(self).ordered();

        // Code is literal and a caption is written between brackets, so marks
        // arriving from a paste have nowhere to go in either.
        let text = if matches!(start.part, Part::Code | Part::Caption) {
            Text::plain(text.text)
        } else {
            text
        };

        if start.block == end.block && start.part == end.part {
            let at = start.offset + text.text.len();
            self.edit_at(start, |body| {
                body.remove(start.offset..end.offset);
                body.insert(start.offset, &text.text);
                for span in &text.marks {
                    body.marks.push(MarkSpan {
                        range: start.offset + span.range.start..start.offset + span.range.end,
                        mark: span.mark.clone(),
                    });
                }
                body.normalize_marks();
            });
            return Cursor {
                offset: at,
                ..start
            }
            .clamp(self);
        }

        // Across cells of one table the table itself survives: the covered
        // cells are emptied and the shape stays, which is what a spreadsheet
        // selection does and what keeps the columns from collapsing.
        if start.block == end.block {
            for part in self.blocks[start.block].parts() {
                if part < start.part || part > end.part {
                    continue;
                }
                // `remove` clamps, so the open end needs no length.
                let (from, to) = (
                    if part == start.part { start.offset } else { 0 },
                    if part == end.part {
                        end.offset
                    } else {
                        usize::MAX
                    },
                );
                self.edit_at(Cursor::new(start.block, part, 0), |body| {
                    body.remove(from..to)
                });
            }
            return self.replace(Selection::at(start), text);
        }

        // Across blocks the head keeps its kind and takes the tail's
        // remainder, and everything between them goes.
        //
        // A **table is atomic** to a selection that leaves it. Half a table has
        // no shape worth keeping, so an end landing in one takes the whole
        // block rather than splicing a lone cell into a paragraph.
        let head_keeps = !matches!(start.part, Part::Cell { .. })
            && self.blocks[start.block].text_at(start.part).is_some();
        let tail = match end.part {
            Part::Cell { .. } => Text::default(),
            part => self.blocks[end.block]
                .text_at_mut(part)
                .map(|body| body.split_off(end.offset))
                .unwrap_or_default(),
        };

        let indent = self.blocks[start.block].indent;
        let first = if head_keeps {
            start.block + 1
        } else {
            start.block
        };
        self.blocks.drain(first..=end.block);

        let caret = if head_keeps {
            self.edit_at(start, |body| body.remove(start.offset..usize::MAX));
            self.edit_at(start, |body| body.append(tail));
            start
        } else {
            // Everything the selection touched is gone, so the tail arrives as
            // a paragraph in its place.
            self.blocks
                .insert(start.block, Block::at(BlockKind::Paragraph(tail), indent));
            Cursor::new(start.block, Part::Body, 0)
        };
        self.repair();
        let caret = caret.clamp(self);
        self.replace(Selection::at(caret), text)
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
                // A caption lives between brackets, where a line break has no
                // spelling at all.
                BlockKind::Image { alt, .. } => crate::parse::collapse_to_one_line(alt),
                BlockKind::Code { .. } | BlockKind::Bookmark { .. } | BlockKind::Rule => {}
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
    pub fn ceiling(&self, ix: usize) -> u8 {
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
        let span = self.subtree(ix);
        for block in &mut self.blocks[span] {
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
            // Code is literal, so whatever marks the text carried have no
            // meaning inside the fence.
            Self::Code => BlockKind::Code {
                language: None,
                code: Text::plain(text.text),
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

/// A closing inline delimiter just typed, and the run it closes.
///
/// The inline half of the same vocabulary [`shortcut`] covers: typing the last
/// `*` of `**bold**` makes it bold because pasting `**bold**` would have.
/// Returns the opening delimiter's range and the text between it and the caret;
/// the closing delimiter is `inner.end..caret`.
pub fn inline_rule(text: &str, caret: usize) -> Option<(Range<usize>, Range<usize>, Mark)> {
    let head = text.get(..caret)?;
    // Longest first — `**` is bold, and only what is left of it is italic.
    for (delimiter, mark) in [
        ("**", Mark::Bold),
        ("~~", Mark::Strike),
        ("`", Mark::Code),
        ("_", Mark::Italic),
        ("*", Mark::Italic),
    ] {
        let Some(closes) = head.strip_suffix(delimiter) else {
            continue;
        };
        let Some(open) = closes.rfind(delimiter) else {
            continue;
        };
        let inner = open + delimiter.len()..closes.len();
        let Some(body) = text.get(inner.clone()).filter(|body| !body.is_empty()) else {
            continue;
        };
        // Emphasis cannot open or close against whitespace, so a mark reaching
        // over one has no spelling and [`Text::normalize_marks`] would shrink
        // it straight back off. A rule that fires and vanishes is worse than
        // one that does not fire.
        if body.starts_with(char::is_whitespace) || body.ends_with(char::is_whitespace) {
            continue;
        }
        // An underscore inside a word is not emphasis in CommonMark, which is
        // the only reason `snake_case_names` survive being typed.
        if delimiter == "_"
            && text[..open]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric)
        {
            continue;
        }
        return Some((open..open + delimiter.len(), inner, mark));
    }
    None
}
