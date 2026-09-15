//! Host styling for the plain-text source view.

use gpui::{App, Global, Hsla};
use theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourceStyle {
    pub line_numbers: bool,
    /// Minimum number of digit slots; grows to fit the line count.
    pub gutter_min_digits: usize,
    /// Space after the numbers, in multiples of the code font size.
    pub gutter_gap: f32,
    /// `None` uses the current theme's faint text color.
    pub gutter_color: Option<Hsla>,
}

impl Default for SourceStyle {
    fn default() -> Self {
        Self {
            line_numbers: true,
            gutter_min_digits: 1,
            gutter_gap: 1.0,
            gutter_color: None,
        }
    }
}

impl SourceStyle {
    pub fn of(cx: &App) -> Self {
        cx.try_global::<Installed>()
            .map_or_else(Self::default, |installed| (installed.0)(Theme::of(cx)))
    }
}

struct Installed(Box<dyn Fn(&Theme) -> SourceStyle>);

impl Global for Installed {}

/// Resolve styles at paint so host colors follow theme changes.
pub fn set_source_style(cx: &mut App, style: impl Fn(&Theme) -> SourceStyle + 'static) {
    cx.set_global(Installed(Box::new(style)));
    cx.refresh_windows();
}
