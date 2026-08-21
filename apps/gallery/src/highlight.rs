//! The gallery's highlighter, which `markdown` asks for its fenced blocks.
//!
//! Native builds call `syntax` and colour any block on the page. A web build
//! cannot: tree-sitter is C, and there is no libc to compile it against for
//! `wasm32-unknown-unknown`. So `build.rs` — which runs on the host whatever
//! the target is — colours the Syntax page's samples ahead of time, and the
//! browser looks the answer up instead of computing it.

use std::ops::Range;

use theme::HighlightKind;

/// Install with `markdown::set_highlighter(cx, highlight::spans, highlight::languages())`.
#[cfg(not(target_family = "wasm"))]
pub fn spans(language: &str, code: &str) -> Option<Vec<(Range<usize>, HighlightKind)>> {
    syntax::highlight(code, language)
}

/// The names a language picker offers. Whichever grammars the features left in.
#[cfg(not(target_family = "wasm"))]
pub fn languages() -> Vec<&'static str> {
    syntax::lang::LANGS.iter().map(|lang| lang.name).collect()
}

/// The same list, read from the table rather than the grammars — `syntax` is
/// not in the web build to ask.
#[cfg(target_family = "wasm")]
pub fn languages() -> Vec<&'static str> {
    LANGUAGES.to_vec()
}

#[cfg(target_family = "wasm")]
include!(concat!(env!("OUT_DIR"), "/highlights.rs"));

/// A block the build script never saw paints plain — the same thing an unknown
/// language does anyway.
#[cfg(target_family = "wasm")]
pub fn spans(language: &str, code: &str) -> Option<Vec<(Range<usize>, HighlightKind)>> {
    HIGHLIGHTS
        .iter()
        .find(|(tag, sample, _)| *tag == language && *sample == code)
        .map(|(_, _, spans)| spans.to_vec())
}
