//! The washes behind transient find matches.

use gpui::{App, Global, Hsla};
use theme::Theme;

/// Returns `(match, current)` washes for the theme. Keep the current match
/// visibly stronger in both appearances.
pub type FindPaint = fn(theme: &Theme) -> (Hsla, Hsla);

struct Installed(FindPaint);

impl Global for Installed {}

/// Installs the find washes for editors and previews. Without it they use
/// [`default_find`]. Call again to replace the paint when preferences change.
pub(crate) fn set_find_paint(cx: &mut App, paint: FindPaint) {
    cx.set_global(Installed(paint));
}

/// The shipped accent washes: 22% for matches and 48% for the current match.
pub fn default_find(theme: &Theme) -> (Hsla, Hsla) {
    (theme.accent.opacity(0.22), theme.accent.opacity(0.48))
}

pub(crate) fn find_paint_of(cx: &App) -> FindPaint {
    cx.try_global::<Installed>()
        .map_or(default_find, |installed| installed.0)
}
