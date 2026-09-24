//! Finding a range again in a document edited somewhere this crate never saw.
//!
//! A saved [`Selection`] is offsets, and offsets only hold for the document
//! they were taken from. A [`Quote`] is the text under the range with a little
//! context either side — the W3C Web Annotation model's `TextQuoteSelector` —
//! which is what survives the file being rewritten on disk.
//!
//! ```
//! use markdown::{Cursor, Part, Quote, Selection, parse};
//!
//! let before = parse("alpha one\n\nbravo two");
//! let range = Selection::new(Cursor::new(1, Part::Body, 6), Cursor::new(1, Part::Body, 9));
//! let quote = Quote::of(&before, range).unwrap();
//!
//! let after = parse("new first\n\nalpha one\n\nbravo two");
//! let found = quote.find(&after, Some(range)).unwrap();
//! assert_eq!(found.ordered().0, Cursor::new(2, Part::Body, 6));
//! ```

use crate::{Cursor, Doc, Selection};

/// How many characters of context a quote keeps on either side.
const CONTEXT: usize = 32;

/// The text a range covers, and what stands either side of it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Quote {
    pub exact: String,
    /// Up to 32 characters before `exact`, ending where it starts.
    pub prefix: String,
    /// Up to 32 characters after `exact`, starting where it ends.
    pub suffix: String,
}

impl Quote {
    /// The quote under `range`, or `None` for a collapsed range.
    ///
    /// Text parts are read in document order with a newline between each, so a
    /// range across blocks quotes the break as `\n`.
    pub fn of(doc: &Doc, range: Selection) -> Option<Self> {
        let flat = Flat::new(doc);
        let (start, end) = range.clamp(doc).ordered();
        let (start, end) = (flat.index(start)?, flat.index(end)?);
        if start >= end {
            return None;
        }
        let text = flat.text.as_str();
        Some(Self {
            exact: text[start..end].to_string(),
            prefix: tail(&text[..start], CONTEXT).to_string(),
            suffix: head(&text[end..], CONTEXT).to_string(),
        })
    }

    /// Where the quote is in `doc`, or `None` when its text is gone.
    ///
    /// `hint` is the range as last saved. Where it still covers the exact
    /// text, it is the answer. Otherwise every occurrence is weighed by how
    /// much of the prefix and suffix still stand beside it, and a tie goes to
    /// the one nearest the hint.
    pub fn find(&self, doc: &Doc, hint: Option<Selection>) -> Option<Selection> {
        if self.exact.is_empty() {
            return None;
        }
        let flat = Flat::new(doc);
        let text = flat.text.as_str();
        let near = hint.and_then(|hint| flat.index(hint.clamp(doc).ordered().0));
        if let Some(at) = near
            && text
                .get(at..)
                .is_some_and(|rest| rest.starts_with(&self.exact))
        {
            return flat.selection(at, at + self.exact.len());
        }

        let at = occurrences(text, &self.exact).max_by_key(|&at| {
            let end = at + self.exact.len();
            let context =
                common_tail(&text[..at], &self.prefix) + common_head(&text[end..], &self.suffix);
            let distance = near.map_or(0, |near| near.abs_diff(at));
            (context, std::cmp::Reverse(distance))
        })?;
        flat.selection(at, at + self.exact.len())
    }
}

/// Every text part of a document laid end to end.
struct Flat {
    text: String,
    /// Where each part starts in `text`, and the cursor at its offset zero.
    parts: Vec<(usize, Cursor)>,
}

impl Flat {
    fn new(doc: &Doc) -> Self {
        let mut text = String::new();
        let mut parts = Vec::new();
        for (ix, block) in doc.blocks.iter().enumerate() {
            for part in block.parts() {
                let Some(body) = block.text_at(part) else {
                    continue;
                };
                if !parts.is_empty() {
                    text.push('\n');
                }
                parts.push((text.len(), Cursor::new(ix, part, 0)));
                text.push_str(&body.text);
            }
        }
        Self { text, parts }
    }

    /// Where `cursor` falls in the flat text.
    fn index(&self, cursor: Cursor) -> Option<usize> {
        let at = self
            .parts
            .iter()
            .position(|(_, start)| start.block == cursor.block && start.part == cursor.part)?;
        let (start, _) = self.parts[at];
        let end = self
            .parts
            .get(at + 1)
            .map_or(self.text.len(), |(next, _)| next - 1);
        Some((start + cursor.offset).min(end))
    }

    /// The cursor at flat index `at`: in the last part starting at or before it.
    fn cursor(&self, at: usize) -> Option<Cursor> {
        let (start, cursor) = self.parts.iter().rev().find(|(start, _)| *start <= at)?;
        Some(Cursor {
            offset: at - start,
            ..*cursor
        })
    }

    fn selection(&self, start: usize, end: usize) -> Option<Selection> {
        Some(Selection::new(self.cursor(start)?, self.cursor(end)?))
    }
}

/// Every place `exact` starts, overlapping ones included.
fn occurrences<'a>(text: &'a str, exact: &'a str) -> impl Iterator<Item = usize> + 'a {
    text.char_indices()
        .map(|(at, _)| at)
        .filter(move |&at| text[at..].starts_with(exact))
}

/// The last `count` characters of `text`.
fn tail(text: &str, count: usize) -> &str {
    let at = text
        .char_indices()
        .rev()
        .nth(count.saturating_sub(1))
        .map_or(0, |(at, _)| at);
    &text[at..]
}

/// The first `count` characters of `text`.
fn head(text: &str, count: usize) -> &str {
    let at = text
        .char_indices()
        .nth(count)
        .map_or(text.len(), |(at, _)| at);
    &text[..at]
}

/// How many characters `a` and `b` share at their ends.
fn common_tail(a: &str, b: &str) -> usize {
    a.chars()
        .rev()
        .zip(b.chars().rev())
        .take_while(|(a, b)| a == b)
        .count()
}

/// How many characters `a` and `b` share at their starts.
fn common_head(a: &str, b: &str) -> usize {
    a.chars().zip(b.chars()).take_while(|(a, b)| a == b).count()
}
