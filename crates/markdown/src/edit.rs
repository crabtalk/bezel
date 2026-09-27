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

mod shortcut;
mod text;

pub use shortcut::*;

use crate::{
    doc::{Block, BlockKind, Doc, Mark, MarkSpan, Part, Text},
    select::{Cursor, Selection},
};

/// What a [`Doc::replace`] did, for anything holding a position it moved.
///
/// The caret is [`Doc::replace`]'s own answer and every caller wants it. The
/// other two are for a caller keeping a position of its own — a comment anchor,
/// a bookmark into the document — which has no other way to learn that the text
/// under it shifted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Splice {
    /// What the call covered, clamped and in document order.
    pub removed: Selection,
    /// Where the caret landed: the end of what went in.
    pub caret: Cursor,
    /// The change in block count, which every block after [`Self::removed`]
    /// moves by.
    pub blocks: isize,
}

impl Doc {
    /// Split block `ix` at byte offset `at`, returning the new block's index.
    ///
    /// The tail keeps the block's kind where a continued Enter means "another of
    /// these" — a list item, for example. A heading titles what follows. A
    /// quote keeps its kind on whichever side still holds text: split at the
    /// end of one and what opens below is plain body text, split at its start
    /// and what opens above is.
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
            BlockKind::Quote { kind, .. } if !tail.text.is_empty() => BlockKind::Quote {
                kind: *kind,
                text: tail,
            },
            _ => BlockKind::Paragraph(tail),
        };
        // A quote that pushed all its text down keeps none of its own: an
        // alert's marker written twice is two alerts, the first one empty.
        if matches!(kind, BlockKind::Quote { .. })
            && self.blocks[ix]
                .text_at(Part::Body)
                .is_none_or(|text| text.text.is_empty())
        {
            self.blocks[ix].kind = BlockKind::Paragraph(Text::default());
        }
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
            BlockKind::Heading { text, .. } | BlockKind::Quote { text, .. } => Some(text.clone()),
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
                // Stepping into a fence, a caption or a cell leaves this block
                // where it is, which is right while it still holds something
                // and a trap once it does not: nothing above it merges, so a
                // block left empty here is one backspace can never reach again.
                if tail.is_empty() {
                    self.blocks.remove(at.block);
                    self.repair();
                }
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
        let at = self.replace(selection, Text::default()).caret;
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

    /// The marks a selection carries throughout — what a toolbar paints as lit.
    ///
    /// Collapsed, it answers with the marks the next character typed here would
    /// join, which is the **left-sticky** rule [`Text::insert`] already
    /// follows: the run ending at the caret, never the one starting there.
    pub fn marks(&self, selection: Selection) -> Vec<Mark> {
        let mut marks: Vec<Mark> = Vec::new();
        if selection.is_collapsed() {
            let at = selection.head.clamp(self);
            let Some(text) = self.blocks.get(at.block).and_then(|b| b.text_at(at.part)) else {
                return marks;
            };
            for span in &text.marks {
                if span.range.start < at.offset && at.offset <= span.range.end {
                    marks.push(span.mark.clone());
                }
            }
            marks.dedup();
            return marks;
        }
        for (at, range) in self.spans(selection) {
            let Some(text) = self.blocks[at.block].text_at(at.part) else {
                continue;
            };
            for span in &text.marks {
                if span.range.start < range.end
                    && span.range.end > range.start
                    && !marks.contains(&span.mark)
                {
                    marks.push(span.mark.clone());
                }
            }
        }
        marks.retain(|mark| self.covered_by(selection, mark));
        marks
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
            return self.replace(selection, text).caret;
        }

        let caret = self.replace(selection, Text::default()).caret;
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
    pub fn replace(&mut self, selection: Selection, text: Text) -> Splice {
        // An empty document has no block to put anything in; editing one opens
        // the paragraph every other path then assumes exists.
        if self.blocks.is_empty() {
            self.blocks
                .push(Block::new(BlockKind::Paragraph(Text::default())));
        }
        // Counted after that, so the block a nothing-document opens with is not
        // a shift anything downstream has to hear about.
        let before = self.blocks.len();
        let (start, end) = selection.clamp(self).ordered();
        let removed = Selection::new(start, end);

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
            return Splice {
                removed,
                caret: Cursor {
                    offset: at,
                    ..start
                }
                .clamp(self),
                blocks: 0,
            };
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
            // The recursion is the insert alone, so what it covered is not what
            // this call covered — only the caret comes back out of it.
            let caret = self.replace(Selection::at(start), text).caret;
            return Splice {
                removed,
                caret,
                blocks: self.blocks.len() as isize - before as isize,
            };
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
        let caret = self.replace(Selection::at(caret), text).caret;
        Splice {
            removed,
            caret,
            blocks: self.blocks.len() as isize - before as isize,
        }
    }

    /// Put the document into the form markdown can hold — the save step.
    ///
    /// Drops the whitespace markdown discards anyway (leading and trailing on
    /// every line, blank lines at a block's edges), flattens the blocks whose
    /// output is one line, and renumbers ordered runs. After this,
    /// `parse(serialize(doc)) == doc`.
    pub fn normalize(&mut self) {
        self.normalize_with(&crate::Marks::default());
    }

    /// [`Doc::normalize`] with the app's own marks — see [`crate::Marks`].
    pub fn normalize_with(&mut self, marks: &crate::Marks) {
        for block in &mut self.blocks {
            let one_line = matches!(block.kind, BlockKind::Heading { .. });
            match &mut block.kind {
                BlockKind::Paragraph(text)
                | BlockKind::Heading { text, .. }
                | BlockKind::Bullet(text)
                | BlockKind::Ordered { text, .. }
                | BlockKind::Task { text, .. }
                | BlockKind::Quote { text, .. } => {
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
        // different: `# ` and `- ` are both real, so those stay. So is an
        // alert with no body: `> [!TIP]` writes down and reads back.
        self.blocks.retain(|block| {
            !matches!(
                &block.kind,
                BlockKind::Paragraph(text) | BlockKind::Quote { kind: None, text } if text.is_empty()
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
        *self = crate::parse_with(&crate::serialize_with(self, marks), marks);
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
        // A run that has just been nested has nothing above it to carry on
        // from, so it starts over. `renumber` cannot decide this on its own: a
        // list written `5.` keeps its 5, and from the numbers alone the two
        // cases look the same.
        if self.begins_run(ix)
            && let BlockKind::Ordered { number, .. } = &mut self.blocks[ix].kind
        {
            *number = 1;
        }
        self.repair();
        true
    }

    /// Whether the block at `ix` starts a run of ordered items rather than
    /// carrying one on: the nearest block at its own indent, before the list
    /// it sits in ends, is not an ordered item.
    fn begins_run(&self, ix: usize) -> bool {
        let indent = self.blocks[ix].indent;
        self.blocks[..ix]
            .iter()
            .rev()
            .take_while(|block| block.indent >= indent)
            .find(|block| block.indent == indent)
            .is_none_or(|block| !matches!(block.kind, BlockKind::Ordered { .. }))
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
