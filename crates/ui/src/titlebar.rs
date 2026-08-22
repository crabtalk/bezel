//! [`titlebar`] — the strip a window with no system titlebar moves itself by.
//!
//! Two platform facts it exists to carry. The window is moved with
//! `Window::start_window_move` on the first *motion* after a press, never on
//! the press: a bar that moved on mouse-down would swallow every click on the
//! buttons sitting in it. And the macOS traffic lights need
//! [`Theme::TRAFFIC_LIGHT_INSET`] of leading room, which is nothing until the
//! window goes full screen and AppKit takes them away.
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

use gpui::{Div, ElementId, MouseButton, Stateful, Window, div, prelude::*, px};

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
        // The system's own gesture, whatever the user set it to — zoom,
        // minimise or nothing. A no-op off macOS.
        .on_click(|click, window, _| {
            if click.click_count() == 2 {
                window.titlebar_double_click();
            }
        })
}
