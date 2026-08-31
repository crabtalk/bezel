//! How the editor lays the document out.
//!
//! Installed once at boot like the image store, and read at paint: where the
//! document's text sits is the app's decision, and this crate holds only what
//! it defaults to.

use gpui::{App, Global};

/// What the editor lays the document out with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// How far the document's text sits inside the editor's own box, leaving
    /// the drag handle somewhere to be. Anything laid out above or below the
    /// editor takes the same measure to line up with its text; under the
    /// handle's own 18px the handle sits over the text instead of beside it.
    pub text_inset: f32,
}

impl Layout {
    /// How the editor lays out, or [`Layout::default`] before anything is
    /// installed. Mirrors [`theme::Theme::of`].
    pub fn of(cx: &App) -> Self {
        cx.try_global::<Installed>()
            .map_or_else(Self::default, |installed| installed.0)
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self { text_inset: 22.0 }
    }
}

struct Installed(Layout);

impl Global for Installed {}

/// `editor::set_layout(cx, my_layout)` — call once at boot.
pub fn set_layout(cx: &mut App, layout: Layout) {
    cx.set_global(Installed(layout));
}
