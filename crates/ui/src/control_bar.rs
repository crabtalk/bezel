//! [`control_bar`] — the floating control bar: a glass surface holding a
//! leading cluster, an optional centre, and a trailing cluster. Apple Music's
//! transport, a desktop agent app's composer, a floating toolbar.
//!
//! [`Shape`] is the only thing that differs between those; everything below is
//! shape-blind, which is why the module is named for the job rather than for
//! the stadium it started as.
//!
//! Two things it exists to get right.
//!
//! **The blur corners follow the border.** [`crate::material::material`] takes
//! a corner radius and paints the backdrop blur to it, and a mismatch frosts
//! square corners outside a round border. One radius comes out of [`Shape`] and
//! feeds both, so there is no second number to keep in step.
//!
//! **The centre is centred on the bar, not on what the clusters leave.** The
//! two rails are equal-flex and the centre is not: clusters of five controls
//! and three then keep the middle on axis. Flexing the centre between them
//! instead is the classic toolbar bug — it lands wherever the wider cluster
//! pushes it.
//!
//! That second rule is why the bar takes the **width it is given** rather than
//! hugging its controls. Equal rails need free space to be equal *about*; a
//! shrink-to-fit bar has none, and its middle then lands wherever the clusters
//! happen to put it. So width is the caller's, and a `max_w` is how a wide
//! window gets a floating bar instead of a docked one.
//!
//! Placement is the caller's too, and it is four lines. This bar floats *over*
//! content and must never reflow it — the same overlay-never-a-gutter rule
//! [`crate::scroll`] follows. A bar that *does* reflow its content is a dock,
//! not this: no blur, no float, and nothing here to reuse.
//!
//! ```ignore
//! div().relative().size_full()
//!     .child(page)
//!     .child(
//!         div().absolute().bottom(px(20.0)).left_0().right_0()
//!             .flex().justify_center()
//!             .child(div().w_full().max_w(px(880.0)).child(
//!                 control_bar::control_bar(&theme, Shape::Pill, leading, Some(centre), trailing),
//!             )),
//!     )
//! ```

use bezel_theme::{Theme, hairline};
use gpui::{AnyElement, IntoElement, ParentElement as _, Styled as _, div, px};

/// Height of the bar, and so half the radius of a [`Shape::Pill`]. One number
/// rather than a parameter: a stadium's radius has to be derived from it for
/// the material's blur to match the border, and a caller free to pick a height
/// is a caller free to get that wrong.
pub const BAR_HEIGHT: f32 = 56.0;

/// Gap between controls in a cluster, and the bar's own end inset. The inset
/// matches the gap so the first control sits as far from the bar's edge as it
/// does from its neighbour.
const BAR_GAP: f32 = 8.0;

/// How the bar's corners are cut. Two named cases rather than a radius, because
/// this is a choice between two shapes and not a continuum — and because a bare
/// number at the call site says nothing about which one you meant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// A stadium — the radius is half the height. Apple Music's transport.
    Pill,
    /// A rounded rectangle at [`Theme::BUBBLE_RADIUS`], the radius this library
    /// already gives floating rounded things. What most composers want; a
    /// stadium reads as a media control and a composer is not one.
    Rounded,
}

impl Shape {
    fn radius(self) -> f32 {
        match self {
            Shape::Pill => BAR_HEIGHT / 2.0,
            Shape::Rounded => Theme::BUBBLE_RADIUS,
        }
    }
}

/// The bar. `centre` is optional — a toolbar with only clusters passes `None`
/// and the rails still hold their ends.
pub fn control_bar(
    theme: &Theme,
    shape: Shape,
    leading: Vec<AnyElement>,
    centre: Option<AnyElement>,
    trailing: Vec<AnyElement>,
) -> AnyElement {
    let radius = shape.radius();

    let rail = || div().flex().flex_row().items_center().gap(px(BAR_GAP));

    let bar = div()
        .h(px(BAR_HEIGHT))
        .rounded(px(radius))
        .border_1()
        .border_color(hairline(0.10))
        .shadow_lg()
        .overflow_hidden()
        .px(px(BAR_GAP))
        .flex()
        .flex_row()
        .items_center()
        .text_size(px(13.0))
        .text_color(theme.text)
        .bg(if theme.is_glass() {
            theme.glass_overlay()
        } else {
            theme.surface_overlay
        })
        .child(rail().flex_1().justify_start().children(leading))
        .children(centre.map(|centre| div().flex_none().px(px(BAR_GAP)).child(centre)))
        .child(rail().flex_1().justify_end().children(trailing));

    crate::material::material(radius, crate::material::MENU_BLUR, bar).into_any_element()
}

/// A circular control inside a bar: the ring, and its glyph at half the
/// diameter. `diameter` is a parameter because a transport's primary action is
/// deliberately bigger than its neighbours — that size difference is what makes
/// the cluster readable at a glance.
///
/// It builds the icon rather than taking one, the way [`row_tile`](crate::widgets::Scaffolding::row_tile)
/// does, because `tint` is not optional in the way it looks: gpui reads an
/// svg's colour off that element's own style and paints **nothing** when it is
/// unset, so a colour set on this button would silently not reach the glyph.
///
/// Caller adds id, click and its own `.hover(..)`: gpui panics on a second
/// hover call, and the wash differs by state (a lit toggle is not a resting
/// one). [`Theme::glass_hover`] is the wash to reach for.
pub fn bar_button(icon: &'static str, diameter: f32, tint: gpui::Hsla) -> gpui::Div {
    div()
        .flex_none()
        .size(px(diameter))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(
            crate::icons::icon(icon)
                .size(px(diameter / 2.0))
                .text_color(tint),
        )
}
