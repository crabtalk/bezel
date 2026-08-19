//! Tree-sitter syntax classification for isolated code blocks.
//!
//! [`highlight`] takes a source string and a fence tag and returns the
//! highlighted spans as `(byte range, kind)` pairs, in document order.
//! Everything outside those spans is unhighlighted text. There is no color
//! and no rendering here — kinds map to colors through
//! [`SyntaxPalette::color`](theme::SyntaxPalette::color) — and no injection
//! machinery: the fence already names the grammar, so a block is one parse
//! with one highlights query. Languages are a table ([`lang::LANGS`], one row
//! per feature); a grammar with no match returns `None` and the caller renders
//! plain text.
//!
//! A language the table does not carry is a [`Lang::new`](lang::Lang::new)
//! `static` of your own, highlighted through
//! [`Lang::highlight`](lang::Lang::highlight) — the same path the built-in rows
//! take, so nothing about the query cache or the capture vocabulary has to be
//! rebuilt to add one.

use std::ops::Range;
use theme::HighlightKind;

pub mod lang;

/// The exact tree-sitter these grammars were built against. Reach for a
/// `LanguageFn` through here rather than declaring your own tree-sitter, or
/// [`Lang::new`](lang::Lang::new) will not accept it — two versions in the
/// graph are two unrelated types with one name.
pub use tree_sitter;
pub use tree_sitter_language;

/// Highlight `source` as `language` (a fence tag — `rs`, `py`, `tsx`, …).
/// `None` when the tag names no language.
pub fn highlight(source: &str, language: &str) -> Option<Vec<(Range<usize>, HighlightKind)>> {
    lang::resolve(language)?.highlight(source)
}
