//! The glyphs the editor's handles paint: the block's gutter handle and a
//! table's row and column handles. The app installs its own with
//! [`crate::AppExt::set_handles`]; [`Handles::default`] until it does.

use gpui::{App, Global};
use ui::icons::{Icon, glyph};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handles {
    pub block: Icon,
    pub row: Icon,
    pub column: Icon,
}

impl Default for Handles {
    fn default() -> Self {
        Self {
            block: glyph::GripVertical.into(),
            row: glyph::GripVertical.into(),
            column: glyph::GripHorizontal.into(),
        }
    }
}

pub(crate) struct Installed(pub Handles);

impl Global for Installed {}

/// The glyphs the app installed, or [`Handles::default`].
pub(crate) fn installed(cx: &App) -> Handles {
    cx.try_global::<Installed>()
        .map_or_else(Handles::default, |Installed(handles)| handles.clone())
}
