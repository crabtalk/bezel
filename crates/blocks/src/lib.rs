//! Painted fenced blocks for `markdown`.
//!
//! ```ignore
//! markdown::set_block_renderer(cx, blocks::render);   // once, at boot
//! ```
//!
//! A fence already round trips byte for byte, already holds a caret, and
//! already degrades to its own source where nothing paints it — so a block of
//! an app's own is a renderer over ` ```chart ` rather than a new
//! `markdown::BlockKind`. This crate is one answer to that seam; an app with
//! its own block writes the same function and never depends on this.
//!
//! It is a peer crate rather than a feature on `markdown` for the reason
//! `syntax` is: cargo features are additive across the whole graph, so a
//! `markdown/mermaid` any dependency turned on is one no consumer can turn back
//! off, and a block carrying a parser would break a target nobody asked about.
//! A crate you do not name costs nothing.
//!
//! `markdown` is not a dependency here. A renderer is a fence tag and a string
//! in, an element out, which needs no document model — the consumer's call to
//! `set_block_renderer` is what pins the signature.

use gpui::{AnyElement, App, Window};

#[cfg(feature = "chart")]
pub mod chart;

/// Paint the block a fence names, or `None` to leave it to the ordinary code
/// block.
///
/// One answer for a tag no enabled block claims, a block turned off at compile
/// time, and a block that read the source and declined.
pub fn render(language: &str, code: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    match language {
        #[cfg(feature = "chart")]
        chart::LANGUAGE => chart::render(code, window, cx),
        // Spelled out rather than bare, so that turning every block off leaves
        // a signature nothing reads and no warning about it.
        _ => {
            let _ = (code, window, cx);
            None
        }
    }
}

/// The fence tags the enabled blocks answer to, for a language picker that
/// would otherwise offer a block this build cannot paint.
pub fn languages() -> &'static [&'static str] {
    &[
        #[cfg(feature = "chart")]
        chart::LANGUAGE,
    ]
}
