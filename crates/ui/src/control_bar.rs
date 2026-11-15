//! Floating controls with equal leading and trailing rails.
//! Content is built within the bar's material appearance scope.

use gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement as _, Styled as _, Window, div, px,
};
use icons::Icon;
use theme::{TextStyle, Theme, Typeset};

use crate::surface;

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
    /// A rounded rectangle at [`Theme::bubble_radius`], the radius this library
    /// already gives floating rounded things. What most composers want; a
    /// stadium reads as a media control and a composer is not one.
    Rounded,
}

impl Shape {
    fn radius(self) -> f32 {
        match self {
            Shape::Pill => BAR_HEIGHT / 2.0,
            Shape::Rounded => Theme::bubble_radius(),
        }
    }
}

/// The bar. `centre` is optional — a toolbar with only clusters passes `None`
/// and the rails still hold their ends.
pub fn control_bar(
    id: impl Into<ElementId>,
    shape: Shape,
    build: impl FnOnce(
        &Theme,
        &mut Window,
        &mut App,
    ) -> (Vec<AnyElement>, Option<AnyElement>, Vec<AnyElement>)
    + 'static,
) -> AnyElement {
    surface::popover(id, shape.radius(), move |theme, window, cx| {
        let (leading, centre, trailing) = build(theme, window, cx);
        let radius = shape.radius();
        let native = window.native_surfaces.materials && theme.glass;

        let rail = || div().flex().flex_row().items_center().gap(px(BAR_GAP));

        let bar = div()
            .h(px(BAR_HEIGHT))
            .rounded(px(radius))
            .border_1()
            .border_color(if native {
                gpui::transparent_black()
            } else {
                theme.border
            })
            .overflow_hidden()
            .px(px(BAR_GAP))
            .flex()
            .flex_row()
            .items_center()
            .text_style(TextStyle::Body)
            .text_color(theme.text)
            .child(rail().flex_1().justify_start().children(leading))
            .children(centre.map(|centre| div().flex_none().px(px(BAR_GAP)).child(centre)))
            .child(rail().flex_1().justify_end().children(trailing));

        let bar = if native { bar } else { bar.shadow_lg() };
        bar
    })
    .into_any_element()
}

/// A circular control inside a bar: the ring, and its glyph at half the
/// diameter. `diameter` is a parameter because a transport's primary action is
/// deliberately bigger than its neighbours — that size difference is what makes
/// the cluster readable at a glance.
///
/// It builds the icon rather than taking one, the way [`row_icon`](crate::widgets::Scaffolding::row_icon)
/// does, because `tint` is not optional in the way it looks: gpui reads an
/// svg's colour off that element's own style and paints **nothing** when it is
/// unset, so a colour set on this button would silently not reach the glyph.
///
/// Caller adds id, click and its own `.hover(..)`: gpui panics on a second
/// hover call, and the wash differs by state (a lit toggle is not a resting
/// one). [`Theme::element_hover`] is the wash to reach for.
pub fn bar_button(icon: impl Into<Icon>, diameter: f32, tint: gpui::Hsla) -> gpui::Div {
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
