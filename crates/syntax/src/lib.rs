//! Tree-sitter syntax classification.
//!
//! Spans come back in document order, and anything they do not cover is
//! unhighlighted text. No color and no rendering here — kinds map to colors
//! through [`SyntaxPalette::color`](theme::SyntaxPalette::color).
//!
//! [`lang::LANGS`] is one row per cargo feature, fixed at build time; it seeds
//! [`registry`], which also holds the languages this build can name and not
//! paint, and which [`registry::register`] adds to at runtime.
//!
//! A language may carry an injections query marking regions written in another
//! language — `<script>` and `<style>` in html. The injected name is resolved
//! through [`registry`], so it must be one this build can paint.
//!
//! Every language is configured with [`lang::NAMES`]. A `Highlight` index means
//! whatever the layer that produced it was configured with, and an injected
//! parse returns indices of its own.
//!
//! No locals support: that query is passed empty in
//! [`Lang::compiled`](lang::Lang::compiled).

use std::ops::Range;
use theme::HighlightKind;

pub mod lang;
pub mod registry;

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
