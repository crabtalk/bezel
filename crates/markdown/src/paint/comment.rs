//! The washes behind comment ranges.

use gpui::{App, Global, Hsla};
use theme::Theme;

/// The washes a comment's range is painted in, by its state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommentWashes {
    /// The colour the washes are cut from, at full strength.
    pub color: Hsla,
    /// A comment still open.
    pub open: Hsla,
    /// The comment the reader has in front of them.
    pub active: Hsla,
    /// A resolved comment.
    pub resolved: Hsla,
}

impl CommentWashes {
    /// The shipped strengths of `color`: 20% open, 38% active and 8% resolved.
    pub fn of(color: Hsla) -> Self {
        Self {
            color,
            open: color.opacity(0.20),
            active: color.opacity(0.38),
            resolved: color.opacity(0.08),
        }
    }
}

/// Returns the comment washes for the theme. Keep the active one visibly
/// stronger than the open ones in both appearances.
pub type CommentPaint = fn(theme: &Theme) -> CommentWashes;

struct Installed(CommentPaint);

impl Global for Installed {}

/// Installs the comment washes for editors and previews. Without it they use
/// [`default_comment`]. Call again to replace the paint when preferences
/// change.
pub(crate) fn set_comment_paint(cx: &mut App, paint: CommentPaint) {
    cx.set_global(Installed(paint));
}

/// The shipped washes, cut from the theme's warning colour.
pub fn default_comment(theme: &Theme) -> CommentWashes {
    CommentWashes::of(theme.warning)
}

pub(crate) fn comment_paint_of(cx: &App) -> CommentPaint {
    cx.try_global::<Installed>()
        .map_or(default_comment, |installed| installed.0)
}
