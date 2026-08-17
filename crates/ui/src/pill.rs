//! [`pill`] — the floating control bar: a glass stadium holding a leading
//! cluster, an optional centre, and a trailing cluster. A media transport, a
//! desktop agent app's chat input, a floating toolbar — one shape.
//!
//! Two things it exists to get right.
//!
//! **The blur corners follow the height.** [`crate::material::material`] takes
//! a corner radius and paints the backdrop blur to it; a stadium's is half its
//! height, and a mismatch frosts square corners outside a round border. The
//! radius is derived from [`PILL_HEIGHT`] here so no caller has to know that.
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
//! window gets a floating pill instead of a bottom bar.
//!
//! Placement is the caller's too, and it is four lines. A pill floats *over*
//! content and must never reflow it — the same overlay-never-a-gutter rule
//! [`crate::scroll`] follows:
//!
//! ```ignore
//! div().relative().size_full()
//!     .child(page)
//!     .child(
//!         div().absolute().bottom(px(20.0)).left_0().right_0()
//!             .flex().justify_center()
//!             .child(div().w_full().max_w(px(880.0)).child(
//!                 pill::pill(&theme, leading, Some(centre), trailing),
//!             )),
//!     )
//! ```

use bezel_theme::{Theme, hairline};
use gpui::{AnyElement, IntoElement, ParentElement as _, Styled as _, div, px};

/// Height of the bar, and so half of its corner radius. One number rather than
/// a parameter: the radius has to be derived from it for the material's blur to
/// match the border, and a caller free to pick a height is a caller free to get
/// that wrong.
pub const PILL_HEIGHT: f32 = 56.0;

/// Gap between controls in a cluster, and the bar's own end inset. The inset
/// matches the gap so the first control's circle sits as far from the stadium's
/// edge as it does from its neighbour.
const PILL_GAP: f32 = 8.0;

/// The bar. `centre` is optional — a toolbar with only clusters passes `None`
/// and the rails still hold their ends.
pub fn pill(
    theme: &Theme,
    leading: Vec<AnyElement>,
    centre: Option<AnyElement>,
    trailing: Vec<AnyElement>,
) -> AnyElement {
    let radius = PILL_HEIGHT / 2.0;

    let rail = || div().flex().flex_row().items_center().gap(px(PILL_GAP));

    let bar = div()
        .h(px(PILL_HEIGHT))
        .rounded(px(radius))
        .border_1()
        .border_color(hairline(0.10))
        .shadow_lg()
        .overflow_hidden()
        .px(px(PILL_GAP))
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
        .children(centre.map(|centre| div().flex_none().px(px(PILL_GAP)).child(centre)))
        .child(rail().flex_1().justify_end().children(trailing));

    crate::material::material(radius, crate::material::MENU_BLUR, bar).into_any_element()
}

/// A circular control inside a pill: the ring, and its glyph at half the
/// diameter. `diameter` is a parameter because a transport's primary action is
/// deliberately bigger than its neighbours — that size difference is what makes
/// the cluster readable at a glance.
///
/// It builds the icon rather than taking one, the way [`crate::widgets::row_tile`]
/// does, because `tint` is not optional in the way it looks: gpui reads an
/// svg's colour off that element's own style and paints **nothing** when it is
/// unset, so a colour set on this button would silently not reach the glyph.
///
/// Caller adds id, click and its own `.hover(..)`: gpui panics on a second
/// hover call, and the wash differs by state (a lit toggle is not a resting
/// one). [`Theme::glass_hover`] is the wash to reach for.
pub fn pill_button(icon: &'static str, diameter: f32, tint: gpui::Hsla) -> gpui::Div {
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
