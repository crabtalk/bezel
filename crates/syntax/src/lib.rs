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
//! No injection support: a document is one parse with one grammar, so a region
//! written in another language — `<script>` and `<style>` in Svelte, Vue or
//! HTML — is left unhighlighted. No locals support either. Both queries are
//! passed empty in [`Lang::compiled`](lang::Lang::compiled), and the injection
//! callback in [`Lang::highlight`](lang::Lang::highlight) always returns `None`.

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
