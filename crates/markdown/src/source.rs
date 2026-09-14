//! Colouring markdown *source*.
//!
//! The one language this crate can classify without help: it already parses
//! markdown, and a source view needs colour on the platforms
//! [`crate::highlight`] cannot reach — tree-sitter is C, and a browser has no
//! libc to build it against.
//!
//! Spans are painted into a per-byte map rather than pushed as they arrive,
//! because markdown nests — a link inside a heading inside a quote — and the
//! renderer needs them disjoint and in order. Inner events land last and win.

use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag};
use theme::HighlightKind;

/// The fence tags that mean "this is markdown".
pub const LANGUAGES: [&str; 2] = ["md", "markdown"];

/// Whether a fence tag names markdown.
pub fn is_markdown(language: &str) -> bool {
    LANGUAGES.contains(&language)
}

/// Colour `source` as markdown, in bytes and in document order.
pub fn spans(source: &str) -> Vec<(Range<usize>, HighlightKind)> {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut map: Vec<Option<HighlightKind>> = vec![None; source.len()];
    let mut paint = |range: Range<usize>, kind: HighlightKind| {
        for slot in &mut map[range.start.min(source.len())..range.end.min(source.len())] {
            *slot = Some(kind);
        }
    };

    for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { .. }) => paint(range, HighlightKind::Keyword),
            Event::Start(Tag::BlockQuote(_)) => paint(range, HighlightKind::Comment),
            Event::Start(Tag::CodeBlock(_)) | Event::Code(_) => paint(range, HighlightKind::String),
            Event::Start(Tag::Strong | Tag::Emphasis | Tag::Strikethrough) => {
                paint(range, HighlightKind::Constant);
            }
            // The destination only: a link's label is prose and reads as prose.
            // An autolink has no `](` and is a destination all through.
            Event::Start(Tag::Link { .. } | Tag::Image { .. }) => {
                let at = source[range.clone()]
                    .rfind("](")
                    .map_or(range.start, |ix| range.start + ix);
                paint(at..range.end, HighlightKind::Attribute);
            }
            // The marker, not the item: the text of a list is prose too.
            Event::Start(Tag::Item) => paint(marker(source, range), HighlightKind::Punctuation),
            Event::Start(Tag::Table(_)) => {
                for (ix, _) in source[range.clone()].match_indices('|') {
                    paint(
                        range.start + ix..range.start + ix + 1,
                        HighlightKind::Punctuation,
                    );
                }
            }
            Event::TaskListMarker(_) => paint(range, HighlightKind::Boolean),
            Event::Rule => paint(range, HighlightKind::Punctuation),
            Event::Html(_) | Event::InlineHtml(_) => paint(range, HighlightKind::Tag),
            _ => {}
        }
    }

    // Run-length encoded back out, which is what makes the result disjoint and
    // ordered however deeply the source nested.
    let mut spans: Vec<(Range<usize>, HighlightKind)> = Vec::new();
    for (at, kind) in map.into_iter().enumerate() {
        let Some(kind) = kind else { continue };
        match spans.last_mut() {
            Some((range, last)) if *last == kind && range.end == at => range.end = at + 1,
            _ => spans.push((at..at + 1, kind)),
        }
    }
    spans
}

/// A list item's marker: the indent, the bullet or number, and the space after
/// it. Everything the item's own range holds before its text starts.
fn marker(source: &str, range: Range<usize>) -> Range<usize> {
    let item = &source[range.clone()];
    let text = item.trim_start();
    let start = range.start + (item.len() - text.len());
    let width = text
        .find(char::is_whitespace)
        .map_or(text.len(), |ix| ix + 1);
    start..start + width
}
