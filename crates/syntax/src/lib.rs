//! Tree-sitter syntax classification for isolated code blocks.
//!
//! [`highlight`] takes a source string and a fence tag and returns the
//! highlighted spans as `(byte range, kind)` pairs, in document order.
//! Everything outside those spans is unhighlighted text. There is no color
//! and no rendering here — kinds map to colors through
//! [`SyntaxPalette::color`](theme::SyntaxPalette::color) — and no injection
//! machinery: the fence already names the grammar, so a block is one parse
//! with one highlights query. Languages are a table ([`lang`]); a grammar
//! with no match returns `None` and the caller renders plain text.

use std::ops::Range;
use theme::HighlightKind;
use tree_sitter_highlight::{HighlightEvent, Highlighter};

mod lang;

/// Highlight `source` as `language` (a fence tag — `rs`, `py`, `tsx`, …).
/// `None` when the tag names no language.
pub fn highlight(source: &str, language: &str) -> Option<Vec<(Range<usize>, HighlightKind)>> {
    let compiled = lang::resolve(language)?.compiled()?;
    let config = &compiled.config;
    let mut highlighter = Highlighter::new();
    highlighter.parser().set_language(&config.language).ok()?;
    let mut spans = Vec::new();
    // Nested highlight starts end with `HighlightEnd`; the top of the stack is
    // the kind painting the `Source` ranges that follow it.
    let mut kinds: Vec<HighlightKind> = Vec::new();
    for event in highlighter
        .highlight(config, source.as_bytes(), None, |_| None)
        .ok()?
        .flatten()
    {
        match event {
            HighlightEvent::HighlightStart(hl) => {
                let name = compiled.names.get(hl.0).map(String::as_str).unwrap_or("");
                kinds.push(lang::kind_of(name));
            }
            HighlightEvent::HighlightEnd => {
                kinds.pop();
            }
            HighlightEvent::Source { start, end } => {
                if let Some(&kind) = kinds.last() {
                    spans.push((start..end, kind));
                }
            }
        }
    }
    Some(spans)
}
