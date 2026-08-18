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

    /// The block's editable inline content, if it has any. `None` for the
    /// blocks an editor treats as atomic (code, images, rules) and for tables,
    /// whose cells are edited individually.
    pub fn text(&self) -> Option<&Text> {
        match &self.kind {
            BlockKind::Paragraph(text)
            | BlockKind::Heading { text, .. }
            | BlockKind::Bullet(text)
            | BlockKind::Ordered { text, .. }
            | BlockKind::Task { text, .. }
            | BlockKind::Quote(text) => Some(text),
            BlockKind::Code { .. }
            | BlockKind::Image { .. }
            | BlockKind::Table { .. }
            | BlockKind::Rule => None,
        }
    }
}

/// The block vocabulary. Closed by design — it is exactly what the slash menu
/// offers, and a consumer that needs a block of its own is a reason to widen
/// this enum rather than to grow an extension system.
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
    Code {
        language: Option<String>,
        code: String,
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
