//! Who paints a fenced block.
//!
//! A consumer that needs a block of its own — a chart, a diagram, an embed —
//! reaches for this rather than for a new [`crate::BlockKind`]. The vocabulary
//! stays closed because markdown is the wire form and a new kind would have to
//! own a syntax; a fence already has one. It round trips byte for byte, holds a
//! caret in [`crate::Part::Code`], and on a build that installs nothing it
//! paints the source it always did.
//!
//! The `mermaid` feature paints a ` ```mermaid ` fence the installed renderer
//! leaves, as a diagram.
//!
//! Installed once at boot like the highlighter, and read at paint.

use std::rc::Rc;

use gpui::{AnyElement, App, Global, Window};

/// Replaces the painted fence's code — see [`Fence::rewrite`].
pub type Rewrite = Rc<dyn Fn(String, &mut Window, &mut App)>;

/// The fence a [`BlockRenderer`] is asked to paint.
pub struct Fence<'a> {
    /// The info string.
    pub language: &'a str,
    pub code: &'a str,
    /// Writes new code into this fence through the editor holding it, as an
    /// undoable edit. `None` in a document nobody is editing.
    pub rewrite: Option<Rewrite>,
}

/// Paints the block a fence's info string names, or `None` to leave it to the
/// ordinary code block.
///
/// One answer for a language nothing paints, a renderer that has not been
/// installed, and a renderer that looked at the code and declined.
///
/// The element sits inside [`crate::render::PAINTED_CONTEXT`]. A press it does
/// not stop reaches the editor, which puts the caret in the fence and shows its
/// source.
pub type BlockRenderer = fn(&Fence<'_>, &mut Window, &mut App) -> Option<AnyElement>;

struct Installed(BlockRenderer);

impl Global for Installed {}

/// `cx.set_block_renderer(my_blocks)` — call once at boot.
pub(crate) fn set_block_renderer(cx: &mut App, renderer: BlockRenderer) {
    cx.set_global(Installed(renderer));
}

pub(crate) fn render(fence: &Fence<'_>, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    // Copied out before the call: the renderer reads the theme and its own
    // globals off the same `cx` this borrows.
    if let Some(renderer) = cx.try_global::<Installed>().map(|installed| installed.0)
        && let Some(painted) = renderer(fence, window, cx)
    {
        return Some(painted);
    }
    #[cfg(feature = "mermaid")]
    if fence.language == crate::mermaid::LANGUAGE {
        return crate::mermaid::render(fence.code, cx);
    }
    None
}
