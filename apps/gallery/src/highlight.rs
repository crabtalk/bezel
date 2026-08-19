//! The gallery's highlighter, which `markdown` asks for its fenced blocks.
//!
//! Native builds call `syntax` and colour any block on the page. A web build
//! cannot: tree-sitter is C, and there is no libc to compile it against for
//! `wasm32-unknown-unknown`. So `build.rs` — which runs on the host whatever
//! the target is — colours the Syntax page's samples ahead of time, and the
//! browser looks the answer up instead of computing it.

use std::ops::Range;

use theme::HighlightKind;

/// Install with `markdown::set_highlighter(cx, highlight::spans)`.
#[cfg(not(target_family = "wasm"))]
pub fn spans(language: &str, code: &str) -> Option<Vec<(Range<usize>, HighlightKind)>> {
    syntax::highlight(code, language)
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
