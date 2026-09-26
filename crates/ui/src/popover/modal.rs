//! Modals, and sheets pinned to an edge.

use super::*;

/// Modal/overlay scrim at the *current* appearance, quoted in dark-mode terms
/// like [`ink`]/[`hairline`] — for callers (`modal`, the attachment lightbox)
/// that paint from a `deferred`/`anchored` layer with no `Theme`/`cx` in
/// scope. Mirrors [`Theme::scrim`], which is pinned at `X = 0.6` dark /
/// `0.32` light; other dark-mode alphas scale the light side by the same
/// ratio so the *dark* result is always exactly `alpha_dark` (never routed
/// through [`Hsla::opacity`], whose `0..=1` clamp would clip a
/// larger-than-0.6 alpha before it could scale the light side).
pub(crate) fn scrim_alpha(alpha_dark: f32) -> gpui::Hsla {
    theme::scrim(alpha_dark)
}

/// Full-window modal: dim scrim + centered card with the `dialog-in` entrance.
/// The scrim swallows clicks; the caller wires its own dismiss/confirm.
/// `viewport` is the window size (an `anchored` layer sizes to its children,
/// so the scrim needs explicit dimensions). The frost radius matches
/// [`dialog_card`]'s 16px rounding.
/// `on_dismiss` is the scrim press. It is a parameter rather than the caller's
/// `.on_mouse_down_out`, for the same reason [`sheet`]'s is: the scrim lives
/// inside this deferred layer, so nothing outside can reach it. Without it a
/// dialog could not be dismissed by clicking away from it *by any caller* —
/// which is how this one shipped, and what it looked like was a dialog that
/// only closed on its own buttons.
pub fn modal(
    id: impl Into<ElementId>,
    viewport: gpui::Size<Pixels>,
    card: AnyElement,
    on_dismiss: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    modal_with(id, viewport, card, DIALOG_RADIUS, 0.6, on_dismiss)
}

/// [`modal`] for glass-tinted cards (the add-space palette): a LIGHTER scrim,
/// so the material card reads like the popovers — the standard 0.6 dim buried
/// the backdrop hue under the blur and the palette came out a flat grey slab
/// next to the hue-inheriting menus (user report).
///
/// The radius is [`Theme::surface_radius`], not a parameter: a glass-tinted
/// modal *is* a popover surface, and the parameter this used to take carried
/// the doc line "must match the card's rounding" — a footgun handed to the
/// caller in writing.
pub fn modal_glass(
    id: impl Into<ElementId>,
    viewport: gpui::Size<Pixels>,
    card: AnyElement,
    on_dismiss: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    modal_with(
        id,
        viewport,
        card,
        Theme::surface_radius(),
        0.35,
        on_dismiss,
    )
}

pub(super) fn modal_with(
    id: impl Into<ElementId>,
    viewport: gpui::Size<Pixels>,
    card: AnyElement,
    corner_radius: f32,
    scrim: f32,
    on_dismiss: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let card = crate::surface::popover(corner_radius, card).into_any_element();
    gpui::deferred(
        gpui::anchored()
            .position(gpui::point(px(0.0), px(0.0)))
            .child(
                div()
                    .occlude()
                    .w(viewport.width)
                    .h(viewport.height)
                    .bg(scrim_alpha(scrim))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(crate::cover::cover())
                    // On the card's wrapper, not the scrim: a press inside the
                    // card is not "out", so the dialog's own buttons keep
                    // working with no occluding overlay and no propagation
                    // games. The scrim covers the viewport and occludes, so
                    // "outside the card" and "on the scrim" are the same press.
                    .child(motion::dialog_in(
                        id,
                        div().child(card).on_mouse_down_out(on_dismiss),
                    )),
            ),
    )
    .priority(2)
    .into_any_element()
}

// ---------------------------------------------------------------------------
// Sheet — a dialog pinned to an edge
// ---------------------------------------------------------------------------

/// Which edge a [`sheet`] slides in from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    /// Up from the bottom edge, full width — the shape a phone puts a picker
    /// or a share list in, and what a narrow window wants instead of a side
    /// panel that leaves no room for the page behind it.
    Bottom,
}

impl Side {
    /// Which way the panel travels. The extent a [`sheet`] is given is read
    /// along this axis: a width for the vertical edges, a height for the
    /// horizontal one.
    fn axis(self) -> gpui::Axis {
        match self {
            Side::Left | Side::Right => gpui::Axis::Horizontal,
            Side::Bottom => gpui::Axis::Vertical,
        }
    }
}

/// Corner rounding of [`dialog_card`], and of a [`sheet_panel`]'s two inner
/// corners (the two on the window edge are off-screen). One number rather than
/// two that happen to match: a sheet *is* the dialog card, pinned to an edge
/// instead of centred. Read three times over — the card, the sheet panel, and
/// the blur under each — which is exactly why it is not a literal.
pub(super) const DIALOG_RADIUS: f32 = 16.0;

/// The panel body of a [`sheet`]: glass card chrome rounded and hairlined on
/// its *inner* edge only — the corners against the window edge are off-screen
/// — so it reads as pulled out of the window rather than floating near it.
pub fn sheet_panel(theme: &Theme, side: Side) -> gpui::Div {
    let card = div()
        .size_full()
        .flex()
        .flex_col()
        .shadow_lg()
        .text_color(theme.text);
    let card = match side {
        Side::Left => card
            .rounded_r(px(DIALOG_RADIUS))
            .border_r_1()
            .border_color(hairline(0.10)),
        Side::Right => card
            .rounded_l(px(DIALOG_RADIUS))
            .border_l_1()
            .border_color(hairline(0.10)),
        Side::Bottom => card
            .rounded_t(px(DIALOG_RADIUS))
            .border_t_1()
            .border_color(hairline(0.10)),
    };
    if theme.glass {
        card.bg(theme.glass_overlay())
    } else {
        card.bg(theme.surface_overlay)
    }
}

/// A panel over a dim scrim — [`modal`] pinned to an edge. It slides in over
/// [`motion::DIALOG_IN`] and, once the caller's [`Popup`] enters its exit
/// phase, back out over [`motion::MENU_OUT`] — which it must, because
/// [`Popup::finish_close`] reaps on that spec's span.
///
/// `extent` is measured across the edge the sheet is pinned to: a width on
/// [`Side::Left`] and [`Side::Right`], a height on [`Side::Bottom`]. The other
/// axis always spans the viewport.
///
/// `on_dismiss` is the scrim click. Unlike the anchored menus, dismissal
/// cannot be the caller's `.on_mouse_down_out`: the scrim lives inside this
/// deferred layer, so nothing outside can reach it.
///
/// The slide is written here rather than as a `motion` helper because
/// only the *spec* is motion — which inset carries it is layout, and it
/// differs per side.
pub fn sheet(
    id: impl Into<SharedString>,
    viewport: gpui::Size<Pixels>,
    side: Side,
    extent: Pixels,
    content: AnyElement,
    closing: Option<web_time::Instant>,
    on_dismiss: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let id = id.into();
    let exit = closing.map(exit_progress);
    // The panel spans the edge it is pinned to and takes `extent` across it,
    // which is why the argument is not called a width: on [`Side::Bottom`] it
    // is the height.
    let panel = div().absolute();
    let panel = match side.axis() {
        gpui::Axis::Horizontal => panel.top_0().bottom_0().w(extent),
        gpui::Axis::Vertical => panel.left_0().right_0().h(extent),
    };
    let panel = panel.child(crate::surface::popover(DIALOG_RADIUS, content));
    // `t` runs 0 (fully off-screen) → 1 (seated against the edge).
    let seat = move |el: gpui::Div, t: f32| {
        let inset = extent * (t - 1.0);
        match side {
            Side::Left => el.left(inset),
            Side::Right => el.right(inset),
            Side::Bottom => el.bottom(inset),
        }
    };
    let panel = if let Some(t) = exit {
        // The dying panel must not take clicks — same overlay `menu_motion`
        // puts over an exiting menu.
        let panel = seat(panel, 1.0 - t).child(div().absolute().inset_0().occlude());
        panel
            .with_animation(
                SharedString::from(format!("{id}-out")),
                motion::MENU_OUT.animation(),
                move |el, _| el,
            )
            .into_any_element()
    } else {
        panel
            .with_animation(id.clone(), motion::DIALOG_IN.animation(), seat)
            .into_any_element()
    };

    gpui::deferred(
        gpui::anchored()
            .position(gpui::point(px(0.0), px(0.0)))
            .child(
                div()
                    .id(SharedString::from(format!("{id}-scrim")))
                    .occlude()
                    .relative()
                    .w(viewport.width)
                    .h(viewport.height)
                    .bg(scrim_alpha(0.6 * (1.0 - exit.unwrap_or(0.0))))
                    .on_click(on_dismiss)
                    .child(crate::cover::cover())
                    .child(panel),
            ),
    )
    .priority(2)
    .into_any_element()
}
