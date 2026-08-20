//! Positions and ranges in a [`Doc`].
//!
//! A [`Cursor`] is `(block, part, offset)` — which block, which of its texts,
//! and how far into it. The part is a *coordinate*, not a path: a block has one
//! kind of part and never a mix, so the model stays flat while a caret can still
//! reach inside a code block or a table cell.
//!
//! Because the three fields order lexicographically, a [`Selection`] is just two
//! cursors and `min`/`max` decides which end is which. That is what lets every
//! delete, every paste and every keystroke be the same operation —
//! [`Doc::replace`] over a range — rather than a special case per key.
//!
//! This half is pure, so the motion is testable without a window.

use crate::doc::{Doc, Part};

/// A caret: which block, which part of it, and how far into that part.
///
/// Byte offsets, like the marks — and like them, always on a character
/// boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Cursor {
    pub block: usize,
    pub part: Part,
    pub offset: usize,
}

impl Cursor {
    pub fn new(block: usize, part: Part, offset: usize) -> Self {
        Self {
            block,
            part,
            offset,
        }
    }

    /// How much text this position's part holds, or `None` when the block has
    /// no such part — an image and a rule have none at all.
    pub fn len_in(self, doc: &Doc) -> Option<usize> {
        doc.blocks
            .get(self.block)?
            .text_at(self.part)
            .map(|text| text.text.len())
    }

    /// Pull the caret back onto a real position: a block that exists, a part
    /// that block has, an offset inside it, and a character boundary.
    pub fn clamp(self, doc: &Doc) -> Self {
        if doc.blocks.is_empty() {
            return Self::default();
        }
        let block = self.block.min(doc.blocks.len() - 1);
        // The part can vanish under the caret — a table loses a row, a block
        // changes kind — so fall back to the block's first, and to nothing at
        // all for a block no caret can enter.
        let part = match doc.blocks[block].text_at(self.part) {
            Some(_) => self.part,
            None => match doc.blocks[block].parts().first() {
                Some(part) => *part,
                None => return Self::new(block, Part::default(), 0),
            },
        };
        let here = Self::new(block, part, self.offset);
        let Some(text) = doc.blocks[block].text_at(part) else {
            return here;
        };
        let mut offset = self.offset.min(text.text.len());
        while offset > 0 && !text.text.is_char_boundary(offset) {
            offset -= 1;
        }
        Self { offset, ..here }
    }

    /// The first position at or after block `from` a caret can sit in.
    fn next_editable(doc: &Doc, from: usize) -> Option<Self> {
        (from..doc.blocks.len()).find_map(|ix| {
            doc.blocks[ix]
                .parts()
                .first()
                .map(|part| Self::new(ix, *part, 0))
        })
    }

    /// The last position at or before block `from` a caret can sit in, at its
    /// end — where stepping backwards into it lands.
    fn previous_editable(doc: &Doc, from: usize) -> Option<Self> {
        (0..=from.min(doc.blocks.len().saturating_sub(1)))
            .rev()
            .find_map(|ix| {
                let part = *doc.blocks[ix].parts().last()?;
                let at = Self::new(ix, part, 0);
                Some(Self {
                    offset: at.len_in(doc).unwrap_or(0),
                    ..at
                })
            })
    }

    /// The neighbouring part within this block — the cells of a table are the
    /// only case, and they are why this is not just "the next block".
    fn step_part(self, doc: &Doc, by: isize) -> Option<Part> {
        let parts = doc.blocks.get(self.block)?.parts();
        let ix = parts.iter().position(|part| *part == self.part)?;
        parts.get(ix.checked_add_signed(by)?).copied()
    }

    /// One character left, stepping into the part or block before it.
    pub fn left(self, doc: &Doc) -> Self {
        let here = self.clamp(doc);
        if here.offset > 0
            && let Some(text) = doc.blocks[here.block].text_at(here.part)
        {
            let mut offset = here.offset - 1;
            while offset > 0 && !text.text.is_char_boundary(offset) {
                offset -= 1;
            }
            return Self { offset, ..here };
        }
        if let Some(part) = here.step_part(doc, -1) {
            let at = Self { part, ..here };
            return Self {
                offset: at.len_in(doc).unwrap_or(0),
                ..at
            };
        }
        here.block
            .checked_sub(1)
            .and_then(|ix| Self::previous_editable(doc, ix))
            .unwrap_or(here)
    }

    /// One character right, stepping into the part or block after it.
    pub fn right(self, doc: &Doc) -> Self {
        let here = self.clamp(doc);
        let len = here.len_in(doc).unwrap_or(0);
        if here.offset < len
            && let Some(text) = doc.blocks[here.block].text_at(here.part)
        {
            let mut offset = here.offset + 1;
            while offset < len && !text.text.is_char_boundary(offset) {
                offset += 1;
            }
            return Self { offset, ..here };
        }
        if let Some(part) = here.step_part(doc, 1) {
            return Self {
                part,
                offset: 0,
                ..here
            };
        }
        Self::next_editable(doc, here.block + 1).unwrap_or(here)
    }

    /// The row above, keeping the offset where it fits.
    ///
    /// Within a table that is the cell above in the same column; everywhere
    /// else it is the block above. A wrapped paragraph's visual lines are not
    /// reachable from here — that needs the paint's layout, and the editor
    /// resolves it there.
    pub fn up(self, doc: &Doc) -> Self {
        let here = self.clamp(doc);
        if let Part::Cell { row, column } = here.part
            && row > 0
        {
            return Self {
                part: Part::Cell {
                    row: row - 1,
                    column,
                },
                ..here
            }
            .clamp(doc);
        }
        match here
            .block
            .checked_sub(1)
            .and_then(|ix| Self::previous_editable(doc, ix))
        {
            Some(above) => Self {
                offset: here.offset,
                ..above
            }
            .clamp(doc),
            None => Self { offset: 0, ..here },
        }
    }

    /// The row below, keeping the offset where it fits.
    pub fn down(self, doc: &Doc) -> Self {
        let here = self.clamp(doc);
        if let Part::Cell { row, column } = here.part {
            let below = Self {
                part: Part::Cell {
                    row: row + 1,
                    column,
                },
                ..here
            };
            if below.len_in(doc).is_some() {
                return below.clamp(doc);
            }
        }
        match Self::next_editable(doc, here.block + 1) {
            Some(below) => Self {
                offset: here.offset,
                ..below
            }
            .clamp(doc),
            None => Self {
                offset: here.len_in(doc).unwrap_or(0),
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
            offset: here.len_in(doc).unwrap_or(0),
            ..here
        }
    }

    /// The start of the word at or before the caret — alt-left.
    ///
    /// Whitespace first, then the run of word characters, which is the rule
    /// every platform's word-left follows and the one `ui::TextField` uses.
    pub fn word_left(self, doc: &Doc) -> Self {
        let here = self.clamp(doc);
        let Some(text) = doc.blocks[here.block].text_at(here.part) else {
            return here.left(doc);
        };
        if here.offset == 0 {
            return here.left(doc);
        }
        let head = &text.text[..here.offset];
        let trimmed = head.trim_end_matches(|c: char| !c.is_alphanumeric());
        let offset = trimmed.trim_end_matches(char::is_alphanumeric).len();
        Self { offset, ..here }
    }

    /// The end of the word at or after the caret — alt-right.
    pub fn word_right(self, doc: &Doc) -> Self {
        let here = self.clamp(doc);
        let Some(text) = doc.blocks[here.block].text_at(here.part) else {
            return here.right(doc);
        };
        if here.offset >= text.text.len() {
            return here.right(doc);
        }
        let tail = &text.text[here.offset..];
        let skipped = tail.len()
            - tail
                .trim_start_matches(|c: char| !c.is_alphanumeric())
                .len();
        let rest = &tail[skipped..];
        let word = rest.len() - rest.trim_start_matches(char::is_alphanumeric).len();
        Self {
            offset: here.offset + skipped + word,
            ..here
        }
    }
}

/// A range in the document: where the selection started, and where it is being
/// dragged to.
///
/// The head is the end that moves — shift+arrow and a mouse drag both leave the
/// anchor where it was. Collapsed (`anchor == head`) is an ordinary caret, so
/// there is one position type in the editor rather than two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    pub anchor: Cursor,
    pub head: Cursor,
}

impl Selection {
    /// A collapsed selection — a plain caret.
    pub fn at(cursor: Cursor) -> Self {
        Self {
            anchor: cursor,
            head: cursor,
        }
    }

    pub fn new(anchor: Cursor, head: Cursor) -> Self {
        Self { anchor, head }
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.head
    }

    /// The two ends in document order.
    pub fn ordered(&self) -> (Cursor, Cursor) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }

    /// Move the head, leaving the anchor — every shift+motion and every drag.
    pub fn extend_to(self, head: Cursor) -> Self {
        Self { head, ..self }
    }

    pub fn clamp(self, doc: &Doc) -> Self {
        Self {
            anchor: self.anchor.clamp(doc),
            head: self.head.clamp(doc),
        }
    }

    /// The whole document.
    pub fn all(doc: &Doc) -> Self {
        let start = Cursor::next_editable(doc, 0).unwrap_or_default();
        let end =
            Cursor::previous_editable(doc, doc.blocks.len().saturating_sub(1)).unwrap_or(start);
        Self::new(start, end)
    }
}
