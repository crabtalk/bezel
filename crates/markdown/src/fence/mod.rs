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

mod chrome;
pub mod highlight;
#[cfg(feature = "mermaid")]
pub mod mermaid;

use std::rc::Rc;

use gpui::{AnyElement, App, Global, SharedString, Window};

pub(crate) use chrome::{CODE_PADDING_X, CODE_PADDING_Y, copy_button};
pub use chrome::{NOTE_SELECTOR, fence_band, fence_note, fence_panel};

/// Replaces the painted fence's code — see [`Fence::rewrite`].
pub type Rewrite = Rc<dyn Fn(String, &mut Window, &mut App)>;

/// The fence a [`BlockRenderer`] is asked to paint.
pub struct Fence<'a> {
    /// The info string.
    pub language: &'a str,
    pub code: &'a str,
    /// The height to stand at, in whole pixels, from the fence's info string
    /// or a resize in flight. `None` is the block's own height.
    pub height: Option<u32>,
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

/// What a fence is painted as: a block in its place, or its code with a note
/// under it.
pub(crate) enum Painted {
    Block(AnyElement),
    Code(Option<SharedString>),
}

/// `block` is the fence's place in its document.
pub(crate) fn render(
    fence: &Fence<'_>,
    #[cfg_attr(not(feature = "mermaid"), allow(unused_variables))] block: usize,
    window: &mut Window,
    cx: &mut App,
) -> Painted {
    // Copied out before the call: the renderer reads the theme and its own
    // globals off the same `cx` this borrows.
    if let Some(renderer) = cx.try_global::<Installed>().map(|installed| installed.0)
        && let Some(painted) = renderer(fence, window, cx)
    {
        return Painted::Block(painted);
    }
    #[cfg(feature = "mermaid")]
    if fence.language == self::mermaid::LANGUAGE {
        return match self::mermaid::render(fence.code, fence.height, block, window, cx) {
            Ok(painted) => Painted::Block(painted),
            Err(note) => Painted::Code(Some(note)),
        };
    }
    Painted::Code(None)
}
