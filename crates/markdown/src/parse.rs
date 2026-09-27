//! Markdown → [`Doc`].
//!
//! CommonMark nests; a [`Doc`] does not. Indent counts **list nesting only**:
//!
//! - a list item's first paragraph becomes its marker block (bullet, ordered,
//!   task) at one level shallower than the open list count, and anything else
//!   in that item becomes a child at the list count itself;
//! - a blockquote's paragraphs each become a [`BlockKind::Quote`], every one
//!   of them carrying the GFM alert kind the blockquote opened with. Being
//!   inside a quote decides a block's *kind*, never its depth — an indent a
//!   blockquote contributed could not be reproduced in the output, and the
//!   document would move every time it was read;
//! - everything else keeps its kind at the open list count.
//!
//! Mixed containers therefore flatten: `> - a` yields a bullet and loses the
//! quote. That is the cost of the flat model, and the fixed-point test in
//! [`crate::serialize`] is what keeps it from mattering — whatever the first
//! parse decides is stable from then on.
//!
//! The parse also normalizes what markdown itself would not preserve: leading
//! and trailing whitespace per line, blank lines at a block's edges, headings
//! and table cells flattened to one line, and ordered runs renumbered
//! consecutively. Each of those is a place where writing the document back out
//! and reading it again would otherwise land somewhere new.

use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, LinkType, Options, Parser, Tag, TagEnd,
};
use std::ops::Range;

use crate::{
    doc::{Align, Block, BlockKind, Doc, Form, Mark, MarkSpan, QuoteKind, Text},
    marks::Marks,
    select::Cursor,
};

mod inline;
mod lift;
mod state;

pub use inline::*;
pub use lift::*;
use state::*;

/// The extensions this crate reads. [`crate::source`] colours with the same set.
pub(crate) const OPTIONS: Options = Options::ENABLE_TABLES
    .union(Options::ENABLE_STRIKETHROUGH)
    .union(Options::ENABLE_TASKLISTS)
    .union(Options::ENABLE_GFM);

impl From<BlockQuoteKind> for QuoteKind {
    fn from(kind: BlockQuoteKind) -> Self {
        match kind {
            BlockQuoteKind::Note => Self::Note,
            BlockQuoteKind::Tip => Self::Tip,
            BlockQuoteKind::Important => Self::Important,
            BlockQuoteKind::Warning => Self::Warning,
            BlockQuoteKind::Caution => Self::Caution,
        }
    }
}

/// Parse a markdown document.
pub fn parse(source: &str) -> Doc {
    parse_plain(source)
}

/// A parse, and where in the source each block came from.
pub struct ParsedDoc {
    pub doc: Doc,
    /// One range per `doc.blocks` entry, in document order.
    ///
    /// The ranges partition the source: the first starts at 0, each one ends
    /// where the next begins, and the last ends at `source.len()`. Splicing
    /// them back in order reproduces the source byte for byte.
    ///
    /// A block the source spells inside another's bytes — the empty bullet of
    /// `- ![](cover.png)` — takes an empty range. The one per entry holds
    /// either way.
    pub block_ranges: Vec<Range<usize>>,
}

/// [`parse`], keeping the source range each block was parsed from.
pub fn parse_ranges(source: &str) -> ParsedDoc {
    let (doc, starts) = parse_spanned(source);
    ParsedDoc {
        block_ranges: ranges(&starts, source.len()),
        doc,
    }
}

impl From<&str> for Doc {
    fn from(source: &str) -> Self {
        parse(source)
    }
}

impl From<&str> for ParsedDoc {
    fn from(source: &str) -> Self {
        parse_ranges(source)
    }
}

impl From<(&str, &Marks)> for Doc {
    fn from((source, marks): (&str, &Marks)) -> Self {
        parse_with(source, marks)
    }
}

/// Block starts, in document order, to one range each.
///
/// Each block runs to where the next one starts, so the partition is the
/// shape of the loop rather than something the parser has to get right: the
/// first range opens at 0, the last closes at `len`, and a start that arrives
/// behind the one before it takes an empty range instead of a backwards one.
fn ranges(starts: &[usize], len: usize) -> Vec<Range<usize>> {
    let mut out = Vec::with_capacity(starts.len());
    let mut at = 0;
    for &start in starts.iter().skip(1) {
        let start = start.clamp(at, len);
        out.push(at..start);
        at = start;
    }
    if !starts.is_empty() {
        out.push(at..len);
    }
    out
}

/// [`parse`] with the app's own marks — see [`crate::Marks`].
///
/// Registered delimiters are lifted out of the source *before* CommonMark sees
/// it, which is the only place the difference between `==` and `\=\=` still
/// exists: a backslash escape is gone by the time there is a [`Text`] to scan,
/// and a pass over one would read an escaped delimiter back as a mark and move
/// the document on every save.
pub fn parse_with(source: &str, marks: &Marks) -> Doc {
    if marks.is_empty() {
        return parse_plain(source);
    }
    let mut doc = parse_plain(&lift(source, marks));
    for block in &mut doc.blocks {
        for part in block.parts() {
            if let Some(text) = block.text_at_mut(part) {
                settle(text, marks);
            }
        }
    }
    doc
}

fn parse_plain(source: &str) -> Doc {
    parse_spanned(source).0
}

/// The parse every entry point runs, with the offset each block started at.
///
/// `renumber` rewrites numbers and adds no block, so the starts stay one per
/// block — the invariant [`ParsedDoc::block_ranges`] rests on.
fn parse_spanned(source: &str) -> (Doc, Vec<usize>) {
    let mut state = ParseState::default();
    for (event, range) in Parser::new_ext(source, OPTIONS).into_offset_iter() {
        state.event(event, range);
    }
    state.doc.renumber();
    (state.doc, state.starts)
}

/// Accumulates one run of inline content and the marks over it.
#[derive(Default)]
struct TextBuilder {
    text: String,
    marks: Vec<MarkSpan>,
    /// Indices into `marks` for the marks still open, innermost last.
    open: Vec<usize>,
}

impl TextBuilder {
    /// Open a mark at the cursor. Marks land in the list in the order they
    /// open, which is outermost first — the ordering [`crate::serialize`] reads
    /// back to reproduce the nesting.
    fn open(&mut self, mark: Mark) {
        let ix = self.marks.len();
        let at = self.text.len();
        self.marks.push(MarkSpan {
            range: at..at,
            mark,
        });
        self.open.push(ix);
    }

    /// Whether anything at all has accumulated — an image with no alt text is
    /// a mark and no text, and still has to close as a block.
    fn is_empty(&self) -> bool {
        self.text.is_empty() && self.marks.is_empty()
    }

    fn close(&mut self) {
        if let Some(ix) = self.open.pop() {
            self.marks[ix].range.end = self.text.len();
        }
    }

    /// A mark that opens and closes around `s` in one event (inline code).
    fn wrap(&mut self, mark: Mark, s: &str) {
        let start = self.text.len();
        self.text.push_str(s);
        self.marks.push(MarkSpan {
            range: start..self.text.len(),
            mark,
        });
    }

    fn take(&mut self) -> Text {
        self.open.clear();
        let mut text = normalize(
            &std::mem::take(&mut self.text),
            &std::mem::take(&mut self.marks),
        );
        settle_mentions(&mut text);
        linkify(&mut text);
        text
    }
}
