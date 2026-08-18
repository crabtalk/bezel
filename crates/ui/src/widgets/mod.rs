//! Widgets, grouped as catalog traits on `Theme` — import the group, reach
//! the component: `use bezel_ui::widgets::{Content, Controls, Layout,
//! Scaffolding, Status};` → `theme.group_box()`, `theme.tab(..)`.
//!
//! What stays here is deliberately trait-shaped: state flags, pure math, and
//! the hover refinements, none of which reads the theme as a receiver.

use gpui::{div, prelude::*, px};

use bezel_theme::ink;

mod buttons;
mod content;
mod controls;
mod layout;
mod scaffolding;
mod status;

pub use buttons::{ButtonStyle, Buttons};
pub use content::Content;
pub use controls::{Controls, SliderDrag};
pub use layout::{Layout, SplitDrag};
pub use scaffolding::{OPTION_CARD_HEIGHT, OPTION_CARD_RADIUS, Scaffolding};
pub use status::Status;

/// What a control paints in the 1px border it keeps for
/// [`crate::focus::focusable`]'s ring: nothing, until focus fills it.
///
/// Always present, never conditional. gpui sizes border-box, so a border that
/// appeared only on focus would shift the content under it by a pixel — a
/// checkbox whose tick jumps as you tab onto it.
pub(crate) const RING_SLOT: gpui::Hsla = gpui::transparent_black();

/// A flag that follows something else until the user takes it over.
///
/// The rule behind a section that opens itself while work streams in and
/// collapses when it stops: auto-follow is right until the first press, and
/// wrong immediately after — whatever the flag does next, the person who
/// clicked has to win. Nothing agent-shaped about it; a build log that unfolds
/// while it runs and a detail pane that follows the selection both want this.
///
/// It is an `Option<bool>` rather than the two flags it reads as (*touched*,
/// plus the value): "untouched, and here is the manual value" is a state that
/// cannot mean anything, and this way it cannot be written.
///
/// ```ignore
/// let open = self.details.get(self.running);           // paint this
/// // …on the header's click:
/// self.details.toggle(self.running);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Takeover(Option<bool>);

impl Takeover {
    /// What to show: `auto` until the first [`Self::toggle`], the user's own
    /// choice from then on.
    pub fn get(self, auto: bool) -> bool {
        self.0.unwrap_or(auto)
    }

    /// Flip what is currently on screen — which while nobody has touched it is
    /// `auto`, *not* the stored value — and take over from here.
    pub fn toggle(&mut self, auto: bool) {
        self.0 = Some(!self.get(auto));
    }
}

/// Where `pointer` falls along `axis` as a fraction of `bounds` — what a
/// divider dragged there makes the split, and what a slider dragged there makes
/// the value. `Axis::Horizontal` travels in x.
///
/// Clamped to `min..=1-min` — the dead zone a split passes so neither pane can
/// be squeezed away, and the `0.0` a slider passes because it has none. On a
/// zero-extent container the answer is `min`: the frame before layout has run
/// would otherwise divide by zero.
pub fn axis_fraction(
    pointer: gpui::Point<gpui::Pixels>,
    bounds: gpui::Bounds<gpui::Pixels>,
    axis: gpui::Axis,
    min: f32,
) -> f32 {
    let min = min.clamp(0.0, 0.5);
    let (offset, extent) = match axis {
        gpui::Axis::Horizontal => (pointer.x - bounds.left(), bounds.size.width),
        gpui::Axis::Vertical => (pointer.y - bounds.top(), bounds.size.height),
    };
    if extent <= px(0.0) {
        return min;
    }
    (offset / extent).clamp(min, 1.0 - min)
}

/// A small state dot — the "working / idle / failed" bead on a row. Takes the
/// tone from the caller so the meaning stays with the caller's domain.
pub fn status_dot(tone: gpui::Hsla) -> gpui::Div {
    div().flex_none().size(px(6.0)).rounded_full().bg(tone)
}

/// The default card-row hover wash (`hover:bg-white/[0.015]`).
pub fn card_row_hover(s: gpui::StyleRefinement) -> gpui::StyleRefinement {
    s.bg(ink(0.015))
}

/// The default step-row hover wash (`hover:bg-white/[0.03]`).
pub fn step_row_hover(s: gpui::StyleRefinement) -> gpui::StyleRefinement {
    s.bg(ink(0.03))
}

/// The default collapsible-header hover wash (`hover:bg-white/[0.03]`).
pub fn collapsible_header_hover(s: gpui::StyleRefinement) -> gpui::StyleRefinement {
    s.bg(ink(0.03))
}
