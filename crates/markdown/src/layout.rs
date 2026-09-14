//! How a document breaks its lines.
//!
//! Installed once at boot like the typography, and read at paint.

use gpui::{App, Global};

/// How a document breaks its lines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// Whether a line too long for a fence wraps, rather than scrolling
    /// sideways inside it.
    pub wrap_code: bool,
}

impl Layout {
    /// How documents break lines, or [`Layout::default`] before anything is
    /// installed. Mirrors [`theme::Theme::of`].
    pub fn of(cx: &App) -> Self {
        cx.try_global::<Installed>()
            .map_or_else(Self::default, |installed| installed.0)
    }
}

impl Default for Layout {
    /// Wrapping, because a caret is what reads a fence here: a scroller can
    /// hold it off the right edge, where nothing on the page brings it back.
    fn default() -> Self {
        Self { wrap_code: true }
    }
}

struct Installed(Layout);

impl Global for Installed {}

/// `markdown::set_layout(cx, my_layout)` — call once at boot.
pub fn set_layout(cx: &mut App, layout: Layout) {
    cx.set_global(Installed(layout));
}
