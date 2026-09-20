//! [`frame`] — the border, corners, shadow and resize edges of a window the
//! system does not decorate.
//!
//! `Window::window_decorations` answers which of the two is in force, and it
//! is not a target: GNOME has never implemented `xdg-decoration`, so a Wayland
//! window there is told `Client` whatever it asked for, while the same binary
//! under KDE or on X11 is told `Server` and this draws nothing. macOS and
//! Windows always answer `Server`.
//!
//! The window opens transparent or the band shows as a filled rect —
//! `WindowBackgroundAppearance::Opaque` has the compositor paint the whole
//! surface, corners included.
//!
//! ```ignore
//! window::frame(self.content(window, cx), window, cx)
//! ```

use gpui::{
    App, Bounds, CursorStyle, Decorations, Div, HitboxBehavior, IntoElement, MouseButton, Pixels,
    Point, ResizeEdge, Size, Window, canvas, div, point, prelude::*, px, size,
};

use theme::Theme;

/// Wrap a window's root in the frame it owes.
///
/// Under `Decorations::Server` the child is handed back in a full-size div and
/// nothing else is painted. Under `Decorations::Client` it takes a border,
/// [`Theme::surface_radius`] corners and [`theme::surface_shadows`], inside a
/// [`Theme::CLIENT_INSET`] band that resizes the window.
///
/// Must be the window's root element: the resize bands are hit-tested against
/// `Window::window_bounds`, so an origin of anything but the window's own is a
/// band in the wrong place.
///
/// An edge `tiling` reports flush — against a screen edge, or another window
/// in a tile — keeps its square corner and gives up its band.
pub fn frame(child: impl IntoElement, window: &mut Window, cx: &App) -> Div {
    let Decorations::Client { tiling } = window.window_decorations() else {
        return div().size_full().child(child);
    };

    let inset = px(Theme::CLIENT_INSET);
    let radius = px(Theme::surface_radius());
    let border = Theme::of(cx).border;
    window.set_client_inset(inset);

    div()
        .size_full()
        .child(
            canvas(
                move |_, window, _| {
                    let size = window.window_bounds().get_bounds().size;
                    bands(size, inset).map(|(edge, band)| {
                        (edge, window.insert_hitbox(band, HitboxBehavior::Normal))
                    })
                },
                |_, bands, window, _| {
                    for (edge, band) in bands {
                        window.set_cursor_style(cursor(edge), &band);
                    }
                },
            )
            .absolute()
            .size_full(),
        )
        .when(!tiling.top, |frame| frame.pt(inset))
        .when(!tiling.bottom, |frame| frame.pb(inset))
        .when(!tiling.left, |frame| frame.pl(inset))
        .when(!tiling.right, |frame| frame.pr(inset))
        .on_mouse_down(MouseButton::Left, move |event, window, _| {
            let size = window.window_bounds().get_bounds().size;
            if let Some(edge) = resize_edge(event.position, inset, size) {
                window.start_window_resize(edge);
            }
        })
        .child(
            div()
                .size_full()
                // Without this the child's own background paints over the
                // corners the border rounds.
                .overflow_hidden()
                .border_color(border)
                .when(!tiling.top, |surface| surface.border_t(px(1.0)))
                .when(!tiling.bottom, |surface| surface.border_b(px(1.0)))
                .when(!tiling.left, |surface| surface.border_l(px(1.0)))
                .when(!tiling.right, |surface| surface.border_r(px(1.0)))
                .when(!(tiling.top || tiling.left), |surface| {
                    surface.rounded_tl(radius)
                })
                .when(!(tiling.top || tiling.right), |surface| {
                    surface.rounded_tr(radius)
                })
                .when(!(tiling.bottom || tiling.left), |surface| {
                    surface.rounded_bl(radius)
                })
                .when(!(tiling.bottom || tiling.right), |surface| {
                    surface.rounded_br(radius)
                })
                .when(!tiling.is_tiled(), |surface| {
                    surface.shadow(theme::surface_shadows())
                })
                .child(child),
        )
}

/// Which edge a press at `pos` resizes, or `None` for a press on the content.
///
/// `inset` is the band's width, and `size` the whole window's — the band runs
/// inside both, so the corners are `inset` squares and each edge is what is
/// left of that side between them.
pub fn resize_edge(pos: Point<Pixels>, inset: Pixels, size: Size<Pixels>) -> Option<ResizeEdge> {
    let (top, bottom) = (pos.y < inset, pos.y > size.height - inset);
    let (left, right) = (pos.x < inset, pos.x > size.width - inset);

    Some(match (top, bottom, left, right) {
        (true, _, true, _) => ResizeEdge::TopLeft,
        (true, _, _, true) => ResizeEdge::TopRight,
        (_, true, true, _) => ResizeEdge::BottomLeft,
        (_, true, _, true) => ResizeEdge::BottomRight,
        (true, ..) => ResizeEdge::Top,
        (_, true, ..) => ResizeEdge::Bottom,
        (_, _, true, _) => ResizeEdge::Left,
        (_, _, _, true) => ResizeEdge::Right,
        _ => return None,
    })
}

/// The eight bands, as the rects [`resize_edge`] classifies to. Disjoint: the
/// corners take their `inset` square and the edges take what is left, so no
/// press lands in two and the hitboxes need no order between them.
fn bands(window: Size<Pixels>, inset: Pixels) -> [(ResizeEdge, Bounds<Pixels>); 8] {
    let band = |x: Pixels, y: Pixels, width: Pixels, height: Pixels| Bounds {
        origin: point(x, y),
        size: size(width, height),
    };
    let (far_x, far_y) = (window.width - inset, window.height - inset);
    let (span_x, span_y) = (window.width - inset - inset, window.height - inset - inset);
    let zero = px(0.0);

    [
        (ResizeEdge::TopLeft, band(zero, zero, inset, inset)),
        (ResizeEdge::TopRight, band(far_x, zero, inset, inset)),
        (ResizeEdge::BottomLeft, band(zero, far_y, inset, inset)),
        (ResizeEdge::BottomRight, band(far_x, far_y, inset, inset)),
        (ResizeEdge::Top, band(inset, zero, span_x, inset)),
        (ResizeEdge::Bottom, band(inset, far_y, span_x, inset)),
        (ResizeEdge::Left, band(zero, inset, inset, span_y)),
        (ResizeEdge::Right, band(far_x, inset, inset, span_y)),
    ]
}

/// The pointer an edge shows.
fn cursor(edge: ResizeEdge) -> CursorStyle {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
    }
}
