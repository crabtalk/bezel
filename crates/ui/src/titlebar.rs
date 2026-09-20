//! [`titlebar`] — the strip a window with no system titlebar moves itself by.
//!
//! Moving the window is three different things. AppKit drags it itself; Linux
//! is told to with `start_window_move`, on the first *motion* after a press
//! and never on the press, or the bar swallows every click on the buttons
//! sitting in it; Windows implements neither and takes a
//! `WindowControlArea::Drag` instead, which its hit test answers with
//! `HTCAPTION`.
//!
//! That last one makes the whole strip a control area, and the platform takes
//! the first one its hit test lands in — so **an interactive child of the bar
//! must `.occlude()`**, or a click on it drags the window on Windows.
//! [`controls`] does.
//!
//! The macOS traffic lights need [`Theme::TRAFFIC_LIGHT_INSET`] of leading
//! room, which is nothing until the window goes full screen and AppKit takes
//! them away. Off macOS the buttons are the app's to paint: [`controls`] is
//! the cluster, and the frame around it — border, corners, shadow, resize
//! edges — is [`crate::window::frame`].
//!
//! The window it belongs to opens with `appears_transparent: true` and
//! **`app_owns_titlebar_drag: true`** — the second one stops AppKit from
//! dragging the window *and* from delaying titlebar clicks while it waits to
//! see a double-click.
//!
//! ```ignore
//! titlebar::titlebar("titlebar", &self.drag, true, window)
//!     .px(px(8.0))
//!     .child(/* … */)
//! ```

use std::{cell::Cell, rc::Rc};

use gpui::{
    App, Div, ElementId, MAX_BUTTONS_PER_SIDE, MouseButton, Stateful, Window, WindowButton,
    WindowButtonLayout, WindowControlArea, div, prelude::*, px,
};

use theme::Theme;

/// Whether the press on a [`titlebar`] is still a candidate for a window move.
///
/// Shaped like [`crate::scroll::FollowState`] and for the same reason: it
/// mutates through `&self`, so the element carries the whole gesture and the
/// view holds one field.
#[derive(Clone, Default)]
pub struct DragState(Rc<Cell<bool>>);

/// The strip: full width, [`Theme::TITLEBAR_HEIGHT`] tall, dragging its window
/// and zooming it on a double click.
///
/// `traffic_lights` reserves the leading inset for the macOS buttons — pass it
/// on the one strip they sit over, and it stands down in full screen, where
/// they are gone and the gap would be a hole.
pub fn titlebar(
    id: impl Into<ElementId>,
    drag: &DragState,
    traffic_lights: bool,
    window: &Window,
) -> Stateful<Div> {
    let (armed, disarm, release) = (drag.0.clone(), drag.0.clone(), drag.0.clone());
    let moving = drag.0.clone();
    let zoomable = window.window_controls().maximize && window.is_resizable();
    div()
        .id(id)
        .w_full()
        .h(px(Theme::TITLEBAR_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .when(traffic_lights && !window.is_fullscreen(), |bar| {
            bar.pl(px(Theme::TRAFFIC_LIGHT_INSET))
        })
        .on_mouse_down(MouseButton::Left, move |_, _, _| armed.set(true))
        .on_mouse_up(MouseButton::Left, move |_, _, _| release.set(false))
        // A press that leaves the bar is not a window move either — without
        // this the flag survives, and the next stray motion over the bar drags
        // the window with no button held.
        .on_mouse_down_out(move |_, _, _| disarm.set(false))
        .on_mouse_move(move |_, window, _| {
            if moving.replace(false) {
                window.start_window_move();
            }
        })
        // What moves the window on Windows: the area answers `WM_NCHITTEST`
        // with `HTCAPTION`, and the system drag, the snap and the double click
        // follow from that. `start_window_move` above is the Linux path and is
        // not implemented there at all.
        .window_control_area(WindowControlArea::Drag)
        // The window menu the desktop hangs off its own titlebar, where the
        // compositor says there is one. On the press, which is where a context
        // menu belongs, and a no-op on macOS and on Windows, where the system
        // menu comes from the caption hit test instead.
        .when(window.window_controls().window_menu, |bar| {
            bar.on_mouse_down(MouseButton::Right, |event, window, _| {
                window.show_window_menu(event.position);
            })
        })
        .on_click(move |click, window, _| {
            if click.click_count() == 2 {
                // macOS runs whatever the user set the gesture to — zoom,
                // minimise or nothing — and Windows zooms from `HTCAPTION`
                // without being asked. Linux has neither, and only where the
                // window can be zoomed at all.
                match cfg!(any(target_os = "linux", target_os = "freebsd")) {
                    true if zoomable => window.zoom_window(),
                    true => {}
                    false => window.titlebar_double_click(),
                }
            }
        })
}

/// Which end of the bar a caption cluster sits at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptionSide {
    Left,
    Right,
}

/// The caption buttons, for a window whose system caption is gone —
/// `appears_transparent` on Windows, `Decorations::Client` on Linux.
///
/// Call it at both ends of the bar and let the desktop decide which end fills:
/// `App::button_layout` reads GNOME's `gtk-decoration-layout`, and a platform
/// that reports no layout at all puts the three on the right.
///
/// Empty in full screen, and short whatever `Window::window_controls` says the
/// compositor will refuse. Close is never refused.
///
/// The cluster takes its own width in the bar, so nothing is reserved for it
/// the way [`Theme::TRAFFIC_LIGHT_INSET`] is reserved for AppKit's lights —
/// those are painted over the client area by someone else, and these are not.
///
/// ```ignore
/// titlebar::titlebar("titlebar", &self.drag, true, window)
///     .child(titlebar::controls(CaptionSide::Left, window, cx))
///     .child(div().flex_1().child(/* … */))
///     .child(titlebar::controls(CaptionSide::Right, window, cx))
/// ```
pub fn controls(side: CaptionSide, window: &Window, cx: &App) -> Div {
    let row = div().flex().flex_row().items_center().h_full();
    if window.is_fullscreen() {
        return row;
    }

    let allowed = window.window_controls();
    let layout = cx.button_layout().unwrap_or(TRAILING);
    let buttons = match side {
        CaptionSide::Left => layout.left,
        CaptionSide::Right => layout.right,
    };

    buttons
        .into_iter()
        .flatten()
        .filter(|button| match button {
            WindowButton::Close => true,
            WindowButton::Maximize => allowed.maximize,
            WindowButton::Minimize => allowed.minimize,
        })
        .fold(row, |row, button| {
            row.child(caption_button(button, window, cx))
        })
}

/// What a platform with no layout of its own gets: all three, trailing.
const TRAILING: WindowButtonLayout = WindowButtonLayout {
    left: [None; MAX_BUTTONS_PER_SIDE],
    right: [
        Some(WindowButton::Minimize),
        Some(WindowButton::Maximize),
        Some(WindowButton::Close),
    ],
};

/// The caption glyphs are drawn to their own scale, not the type ladder's —
/// Windows sets them at 10px whatever the shell font is doing.
const CAPTION_GLYPH: f32 = 10.0;

/// One caption button: the glyph, the hover wash, and the hitbox the platform
/// reads.
///
/// The click is wired everywhere but Windows, where answering `WM_NCHITTEST`
/// with `HTCLOSE` and friends has already handed the press to the system —
/// acting on it here as well would minimise and restore in one gesture.
fn caption_button(button: WindowButton, window: &Window, cx: &App) -> Stateful<Div> {
    let theme = Theme::of(cx);
    let (area, glyph, hover) = match button {
        WindowButton::Close => (WindowControlArea::Close, icons::glyph::X, theme.danger),
        WindowButton::Minimize => (
            WindowControlArea::Min,
            icons::glyph::Minus,
            theme.element_hover,
        ),
        // The restore mark is two offset squares, which is what `copy` draws.
        WindowButton::Maximize => (
            WindowControlArea::Max,
            match window.is_maximized() {
                true => icons::glyph::Copy,
                false => icons::glyph::Square,
            },
            theme.element_hover,
        ),
    };

    div()
        .id(button.id())
        .flex()
        .items_center()
        .justify_center()
        .w(px(Theme::CAPTION_BUTTON_WIDTH))
        .h_full()
        .hover(|button| button.bg(hover))
        .child(
            icons::icon(glyph)
                .size(px(CAPTION_GLYPH))
                .text_color(theme.text),
        )
        .window_control_area(area)
        // The bar under it is one big `Drag` area, and the platform takes the
        // first control area its hit test lands in. Occluding is what takes
        // the bar out of that answer, so the button is a button and not a
        // handle to drag the window by.
        .occlude()
        .when(!cfg!(target_os = "windows"), |control| {
            control.on_click(move |_, window, _| match button {
                WindowButton::Close => window.remove_window(),
                WindowButton::Minimize => window.minimize_window(),
                WindowButton::Maximize => window.zoom_window(),
            })
        })
}
