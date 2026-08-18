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

mod lang;

use std::ops::Range;

use tree_sitter::Language;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use theme::HighlightKind;

use lang::{kind_of, resolve};

/// Highlight `source` as `language` (a fence tag — `rs`, `py`, `tsx`, …).
/// `None` when the tag names no language.
pub fn highlight(source: &str, language: &str) -> Option<Vec<(Range<usize>, HighlightKind)>> {
    let lang = resolve(language)?;
    let grammar: Language = lang.grammar.into();
    let mut config = HighlightConfiguration::new(grammar, lang.name, lang.query, "", "").ok()?;
    // Recognize exactly the capture names the query uses, so every
    // `Highlight` index resolves straight through `names`. `_`-prefixed
    // names are predicate anchors, never paint — recognizing them would
    // emit their ranges as spans.
    let names: Vec<String> = config
        .query
        .capture_names()
        .iter()
        .map(|s| s.to_string())
        .filter(|s| !s.starts_with('_'))
        .collect();
    config.configure(&names);
    let mut highlighter = Highlighter::new();
    highlighter.parser().set_language(&config.language).ok()?;
    let mut spans = Vec::new();
    // Nested highlight starts end with `HighlightEnd`; the top of the stack is
    // the kind painting the `Source` ranges that follow it.
    let mut kinds: Vec<HighlightKind> = Vec::new();
    for event in highlighter
        .highlight(&config, source.as_bytes(), None, |_| None)
        .ok()?
        .flatten()
    {
        match event {
            HighlightEvent::HighlightStart(hl) => {
                kinds.push(kind_of(names.get(hl.0).map(String::as_str).unwrap_or("")));
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
