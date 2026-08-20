//! The document model.
//!
//! A [`Doc`] is a **flat** list of [`Block`]s with an indent level, not a
//! nested tree. That is Notion's model rather than CommonMark's, and it is the
//! decision the rest of this crate hangs off: editing a flat list means Enter
//! splits, Backspace merges, and Tab indents — all list operations. On a
//! nested tree "the previous block" is a traversal and every edit is a
//! restructure.
//!
//! The trade is that arbitrarily nested CommonMark does not survive a round
//! trip: a list inside a quote inside a list flattens. Notion has the same
//! limitation. What is guaranteed is [`crate::serialize`]'s fixed point —
//! parse, serialize, parse again, and the document is unchanged — so an
//! edit/save cycle never drifts.

use std::ops::Range;

/// A markdown document: blocks in document order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Doc {
    pub blocks: Vec<Block>,
}

impl Doc {
    /// Make each run of ordered items consecutive.
    ///
    /// Markdown honours only the *first* number in a list — `1.` followed by `9.`
    /// renders as 1, 2. Two source lists that used different delimiters (`1.` then
    /// `9)`) are separate lists, but a flat document has no list identity to
    /// preserve, so they serialize as one and the second item's number would move
    /// on the next read. Deciding it here means the document already holds what the
    /// next parse would produce.
    pub(crate) fn renumber(&mut self) {
        // The number owed to the next ordered item at each indent level. A run
        // survives blocks nested under it and ends at anything else.
        let mut expected: Vec<Option<u64>> = Vec::new();
        for block in &mut self.blocks {
            let indent = block.indent as usize;
            expected.truncate(indent + 1);
            expected.resize(indent + 1, None);

            if let BlockKind::Ordered { number, .. } = &mut block.kind {
                if let Some(next) = expected[indent] {
                    *number = next;
                }
                expected[indent] = Some(number.saturating_add(1));
            } else {
                expected[indent] = None;
            }
        }
    }
}

/// One block, and how deeply it is nested.
///
/// `indent` obeys one invariant, established by the parser and relied on by
/// the serializer: the first block is at 0, and no block is more than one
/// level deeper than the block before it. A document that satisfies it always
/// serializes to markdown that parses back to the same indents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    pub indent: u8,
}

impl Block {
    pub fn new(kind: BlockKind) -> Self {
        Self { kind, indent: 0 }
    }

    pub fn at(kind: BlockKind, indent: u8) -> Self {
        Self { kind, indent }
    }

    /// One of the block's editable texts. `None` when the block has no such
    /// part — every block is atomic to some [`Part`], and images and rules are
    /// atomic to all of them.
    pub fn text_at(&self, part: Part) -> Option<&Text> {
        match (&self.kind, part) {
            (
                BlockKind::Paragraph(text)
                | BlockKind::Heading { text, .. }
                | BlockKind::Bullet(text)
                | BlockKind::Ordered { text, .. }
                | BlockKind::Task { text, .. }
                | BlockKind::Quote(text),
                Part::Body,
            ) => Some(text),
            (BlockKind::Code { code, .. }, Part::Code) => Some(code),
            (BlockKind::Table { header, .. }, Part::Cell { row: 0, column }) => header.get(column),
            (BlockKind::Table { rows, .. }, Part::Cell { row, column }) => {
                rows.get(row - 1)?.get(column)
            }
            _ => None,
        }
    }

    pub fn text_at_mut(&mut self, part: Part) -> Option<&mut Text> {
        match (&mut self.kind, part) {
            (
                BlockKind::Paragraph(text)
                | BlockKind::Heading { text, .. }
                | BlockKind::Bullet(text)
                | BlockKind::Ordered { text, .. }
                | BlockKind::Task { text, .. }
                | BlockKind::Quote(text),
                Part::Body,
            ) => Some(text),
            (BlockKind::Code { code, .. }, Part::Code) => Some(code),
            (BlockKind::Table { header, .. }, Part::Cell { row: 0, column }) => {
                header.get_mut(column)
            }
            (BlockKind::Table { rows, .. }, Part::Cell { row, column }) => {
                rows.get_mut(row - 1)?.get_mut(column)
            }
            _ => None,
        }
    }

    /// Every part a caret can sit in, in document order.
    pub fn parts(&self) -> Vec<Part> {
        match &self.kind {
            BlockKind::Paragraph(_)
            | BlockKind::Heading { .. }
            | BlockKind::Bullet(_)
            | BlockKind::Ordered { .. }
            | BlockKind::Task { .. }
            | BlockKind::Quote(_) => vec![Part::Body],
            BlockKind::Code { .. } => vec![Part::Code],
            BlockKind::Table { header, rows, .. } => {
                let mut parts = Vec::new();
                if !header.is_empty() {
                    parts.extend((0..header.len()).map(|column| Part::Cell { row: 0, column }));
                }
                for (ix, row) in rows.iter().enumerate() {
                    parts.extend((0..row.len()).map(|column| Part::Cell {
                        row: ix + 1,
                        column,
                    }));
                }
                parts
            }
            BlockKind::Image { .. } | BlockKind::Rule => Vec::new(),
        }
    }
}

/// Which of a block's texts a caret sits in.
///
/// A block has one kind of part and never a mix — prose blocks have a body, a
/// code block has its code, a table has cells — so this is a coordinate rather
/// than a path, and the model stays flat. The ordering is document order, which
/// is what makes a [`crate::Cursor`] comparable and therefore what makes a
/// selection a range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Part {
    #[default]
    Body,
    Code,
    /// Row 0 is the header row; row `n` is `rows[n - 1]`.
    Cell {
        row: usize,
        column: usize,
    },
}

/// The block vocabulary. Closed by design — a consumer that needs a block of
/// its own is a reason to widen this enum rather than to grow an extension
/// system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph(Text),
    Heading {
        /// 1–6.
        level: u8,
        text: Text,
    },
    Bullet(Text),
    Ordered {
        /// The rendered number. Stored rather than derived so a list starting
        /// at 3 survives the round trip.
        number: u64,
        text: Text,
    },
    Task {
        checked: bool,
        text: Text,
    },
    Quote(Text),
    /// The code carries a [`Text`] like every other editable region, so one
    /// accessor and one edit path cover the whole document. Its marks are
    /// unreachable rather than forbidden: nothing that writes here creates one.
    Code {
        language: Option<String>,
        code: Text,
    },
    Image {
        url: String,
        alt: String,
    },
    Table {
        align: Vec<Align>,
        header: Vec<Text>,
        rows: Vec<Vec<Text>>,
    },
    Rule,
}

/// GFM column alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Inline content: a string, plus marks over byte ranges of it.
///
/// Marks are a separate list rather than flags on a run because an editor has
/// to *map* them through insertions and deletions, and because run flags lose
/// nesting order — under flags `**_x_**` and `_**x**_` are the same value.
/// Here they differ by the order of the two spans, and both survive a round
/// trip.
///
/// A newline in `text` is a line break within the block (markdown's soft or
/// hard break, which this model does not distinguish — neither does Notion).
/// Whether it paints as a break or a space is a rendering decision.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Text {
    pub text: String,
    /// Outermost first. Ranges may overlap and may be identical.
    pub marks: Vec<MarkSpan>,
}

impl Text {
    /// Unmarked text.
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            marks: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkSpan {
    pub range: Range<usize>,
    pub mark: Mark,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mark {
    Bold,
    Italic,
    Strike,
    Code,
    Link(String),
    /// An image among text. [`BlockKind::Image`] is the shape an editor offers;
    /// this is what keeps `see ![x](u) here` from silently becoming a link when
    /// the document is saved.
    Image(String),
}
