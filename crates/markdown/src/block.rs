//! Who paints a fenced block.
//!
//! A consumer that needs a block of its own — a chart, a diagram, an embed —
//! reaches for this rather than for a new [`crate::BlockKind`]. The vocabulary
//! stays closed because markdown is the wire form and a new kind would have to
//! own a syntax; a fence already has one. It round trips byte for byte, holds a
//! caret in [`crate::Part::Code`], and on a build that installs nothing it
//! paints the source it always did.
//!
//! Installed once at boot like the highlighter, and read at paint.

use gpui::{AnyElement, App, Global, Window};

/// Paints the block a fence's info string names, or `None` to leave it to the
/// ordinary code block.
///
/// One answer for a language nothing paints, a renderer that has not been
/// installed, and a renderer that looked at the code and declined.
pub type BlockRenderer =
    fn(language: &str, code: &str, &mut Window, &mut App) -> Option<AnyElement>;

struct Installed(BlockRenderer);

impl Global for Installed {}

/// `markdown::set_block_renderer(cx, my_blocks)` — call once at boot.
pub fn set_block_renderer(cx: &mut App, renderer: BlockRenderer) {
    cx.set_global(Installed(renderer));
}

pub(crate) fn render(
    language: Option<&str>,
    code: &str,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    // Copied out before the call: the renderer reads the theme and its own
    // globals off the same `cx` this borrows.
    let renderer = cx.try_global::<Installed>()?.0;
    renderer(language?, code, window, cx)
}
