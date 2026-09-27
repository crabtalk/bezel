//! Menu layers anchored to a trigger, and their enter and exit motion.

use super::*;

/// Pin a floating layer's origin to the trigger's top-left. The anchored
/// element is absolutely positioned; without explicit insets its *static*
/// position is subject to the trigger's own flex alignment (an `items_center`
/// trigger would vertically center the whole floating layer). A zero-size
/// absolutely-inset wrapper fixes the origin at the corner.
pub(super) fn pinned_layer(layer: AnyElement) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_0()
        .child(layer)
        .into_any_element()
}

/// Eased exit progress (0..=1) for a [`Popup`] closing instant, computed from
/// the wall clock at render time. Monotonic by construction — unlike the
/// animation element's own clock, it can never replay from 0 mid-exit.
pub(super) fn exit_progress(since: web_time::Instant) -> f32 {
    let total = motion::MENU_OUT
        .total()
        .mul_f32(motion::speed_scale())
        .as_secs_f32();
    let raw = if total <= 0.0 {
        1.0
    } else {
        (since.elapsed().as_secs_f32() / total).clamp(0.0, 1.0)
    };
    motion::MENU_OUT.progress(raw)
}

/// The surface under a popover layer, on whichever look [`Theme::menu_style`]
/// names. It needs no exit ramp of its own: the primitive reads the element
/// tree's opacity, so a layer playing `menu_out` fades its surface with
/// everything else in it.
pub(super) fn material_menu(content: AnyElement) -> AnyElement {
    crate::surface::popover(Theme::surface_radius(), content).into_any_element()
}

/// Entrance or exit motion for a popover layer. While exiting (the [`Popup`]
/// closing phase, `exit = Some(progress)`) the content plays
/// [`motion::menu_out`] under a fresh animation id (same-id reuse would
/// inherit the entrance's finished clock and snap to the end state) and gets
/// an occluding overlay on top — the dying menu's rows must not take clicks,
/// and the overlay also keeps stray clicks from reaching whatever sits
/// underneath.
pub(super) fn menu_motion(id: SharedString, exit: Option<f32>, inner: gpui::Div) -> AnyElement {
    if let Some(t) = exit {
        let inner = inner.relative().child(div().absolute().inset_0().occlude());
        motion::menu_out(SharedString::from(format!("{id}-out")), t, inner).into_any_element()
    } else {
        motion::menu_in(id, inner).into_any_element()
    }
}

/// The common floating layer. Trigger wrappers only choose the origin.
pub(super) fn menu_layer(
    id: impl Into<SharedString>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
    anchor: Anchor,
    position: Option<Point<Pixels>>,
    gap: f32,
) -> AnyElement {
    let content = material_menu(content);
    let inner = match anchor {
        Anchor::BottomLeft | Anchor::BottomRight => div().occlude().pb(px(gap)),
        _ => div().occlude().pt(px(gap)),
    }
    .child(content);
    let mut layer = gpui::anchored()
        .anchor(anchor)
        .snap_to_window_with_margin(px(SNAP));
    if let Some(position) = position {
        layer = layer.position(position);
    }
    gpui::deferred(layer.child(menu_motion(id.into(), closing.map(exit_progress), inner)))
        .priority(1)
        .into_any_element()
}

/// Wrap popover content in a floating anchored layer attached to the trigger:
/// the caller `.child(anchored_menu(...))`s this from the trigger element while
/// open. Plays `menu-in` (0.14s fade + 2px drop); `closing` (the [`Popup`]
/// exit phase) swaps in `menu-out`. Dismissal is the caller's
/// `.on_mouse_down_out` on the content. The layer `.occlude()`s: hitboxes are
/// paint-order only in gpui, so without it clicks on menu rows would ALSO fire
/// whatever clickable sits under the floating layer.
pub fn anchored_menu(
    id: impl Into<SharedString>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
) -> AnyElement {
    pinned_layer(menu_layer(id, content, closing, Anchor::TopLeft, None, 6.0))
}

/// [`anchored_menu`] opening DOWNWARD from the trigger's bottom edge — a
/// dropdown proper (the sidebar's space filter). The default variant pins to
/// the trigger's top-left, which reads fine for context-style menus but
/// covers a button-shaped trigger.
pub fn anchored_menu_below(
    id: impl Into<SharedString>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
) -> AnyElement {
    anchored_menu_below_gap(id, content, closing, 6.0)
}

/// [`anchored_menu_below`] with a caller-chosen trigger→card gap — the
/// changes-header dropdowns hang off a tight titlebar band and need more
/// breathing room than the default 6px (user report; t3code sits near 10).
pub fn anchored_menu_below_gap(
    id: impl Into<SharedString>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
    gap: f32,
) -> AnyElement {
    div()
        .absolute()
        .bottom_0()
        .left_0()
        .size_0()
        .child(menu_layer(id, content, closing, Anchor::TopLeft, None, gap))
        .into_any_element()
}

/// The panel a [`crate::menu::Item::Submenu`] row drops, mounted on that row —
/// which must be `relative()`. Its first row lines up with the row that opened
/// it, and the two cards touch on whichever side [`submenu::place`] put it.
/// `chain` is the open menu's, shared by every panel in it.
///
/// No `closing`: a submenu is held open by the cursor, and the cursor is
/// cleared before the menu it hangs in begins its own exit.
pub fn anchored_submenu(
    id: impl Into<SharedString>,
    content: AnyElement,
    chain: &Chain,
) -> AnyElement {
    let content = div().occlude().child(material_menu(content));
    submenu::layer(menu_motion(id.into(), None, content), chain)
}

/// [`anchored_menu`] opening UPWARD from the trigger (composer pickers, the
/// user menu — anything anchored near the window bottom; Radix flips these
/// automatically, gpui's `anchored` needs the side picked).
pub fn anchored_menu_above(
    id: impl Into<SharedString>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
) -> AnyElement {
    pinned_layer(menu_layer(
        id,
        content,
        closing,
        Anchor::BottomLeft,
        None,
        6.0,
    ))
}

/// Open an upward menu at a point inside a relative trigger. Useful for text
/// completions, whose natural anchor is the token/caret rather than the input
/// element's outer edge.
pub fn anchored_menu_above_at(
    id: impl Into<SharedString>,
    position: Point<Pixels>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
) -> AnyElement {
    div()
        .absolute()
        .left(position.x)
        .top(position.y)
        .size_0()
        .child(anchored_menu_above(id, content, closing))
        .into_any_element()
}

/// [`anchored_menu_above`] right-aligned to the trigger's right edge (t3code
/// ComboboxPopup `align="end"` — right-side triggers like the composer's ref
/// picker open leftward instead of running off the window).
pub fn anchored_menu_above_end(
    id: impl Into<SharedString>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .right_0()
        .size_0()
        .child(menu_layer(
            id,
            content,
            closing,
            Anchor::BottomRight,
            None,
            6.0,
        ))
        .into_any_element()
}

/// [`anchored_menu_below`] right-aligned to the trigger's right edge — a row
/// whose menu is opened from a control at its end, so the card drops from that
/// control and opens inward rather than off the trigger's far side.
pub fn anchored_menu_below_end(
    id: impl Into<SharedString>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
) -> AnyElement {
    div()
        .absolute()
        .bottom_0()
        .right_0()
        .size_0()
        .child(menu_layer(
            id,
            content,
            closing,
            Anchor::TopRight,
            None,
            6.0,
        ))
        .into_any_element()
}

/// A floating menu at an explicit window position (context menus). Occludes
/// like [`anchored_menu`] so row clicks never reach elements underneath.
pub fn menu_at(
    id: impl Into<SharedString>,
    position: Point<Pixels>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
) -> AnyElement {
    menu_layer(id, content, closing, Anchor::TopLeft, Some(position), 0.0)
}
