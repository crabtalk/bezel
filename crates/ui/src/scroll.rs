//! A styled scrollbar over gpui's own scroll handles.
//!
//! gpui scrolls a `div` perfectly well and draws nothing while it does, so a
//! bezel app has no way to show how far down it is. This is that bar, and only
//! that bar: the caller keeps its own `overflow_y_scroll` container, because a
//! wrapper that swallowed the content would have to re-implement layout for it.
//!
//! ```ignore
//! div().relative()                                  // the bar is absolute in here
//!     .child(
//!         div()
//!             .id("pane")
//!             .size_full()
//!             .overflow_y_scroll()
//!             .track_scroll(&self.scroll)           // gpui's handle, the app's field
//!             .child(content),
//!     )
//!     .child(scroll::scrollbar("pane-bar", &self.scroll, &self.scroll_bar))
//! ```
//!
//! The bar must span the container it reports on — its track *is* the viewport,
//! in the coordinates [`thumb`] answers in.
//!
//! The geometry is transcribed from zed's own scrollbar (`thumb_ranges` in
//! `crates/ui/src/components/scrollbar.rs`), which is 1722 lines of settings
//! system around the fifteen that matter. Two of gpui's conventions are easy to
//! get backwards and both are load-bearing here: `max_offset` is the *overflow*
//! (content minus viewport, not content), and `offset` is **negative** as you
//! scroll down.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, DragMoveEvent, Empty, MouseButton, Pixels, ScrollHandle, SharedString, Window, canvas,
    div, point, prelude::*, px,
};

use bezel_theme::ink;

/// Shortest a thumb may get, however long the document — below this it stops
/// being something a pointer can catch.
pub const MIN_THUMB: Pixels = px(25.0);
/// Width of the strip the thumb sits in.
const TRACK: f32 = 10.0;
/// Width of the thumb itself, centred in the track.
const THUMB: f32 = 6.0;

/// Where the thumb sits in a track of `viewport` length, as a range from the
/// track's start — or `None` when there is nothing to scroll.
///
/// `None` also covers a viewport of zero (the frame before layout has run) and
/// a thumb that would not fit, which is zed's third guard: with a viewport
/// shorter than [`MIN_THUMB`] a bar would be all thumb and no travel.
pub fn thumb(
    viewport: Pixels,
    max_offset: Pixels,
    offset: Pixels,
    min: Pixels,
) -> Option<Range<Pixels>> {
    if viewport <= px(0.0) || max_offset <= px(0.0) {
        return None;
    }
    let content = viewport + max_offset;
    let size = min.max(viewport * (viewport / content));
    if size > viewport {
        return None;
    }
    // Negative going down, and never past either end — a wheel can overshoot.
    let travelled = offset.clamp(-max_offset, px(0.0)).abs();
    let start = (travelled / max_offset) * (viewport - size);
    Some(start..start + size)
}

/// The inverse: the scroll offset that puts the thumb's top at `top`.
///
/// Negative, because that is the direction gpui counts in, and clamped to the
/// scrollable range so a drag past either end simply stops.
pub fn offset_for_thumb(top: Pixels, viewport: Pixels, max_offset: Pixels, size: Pixels) -> Pixels {
    let travel = viewport - size;
    if travel <= px(0.0) || max_offset <= px(0.0) {
        return px(0.0);
    }
    -(max_offset * (top / travel).clamp(0.0, 1.0))
}

/// The drag payload. Carries the bar's id because, unlike a split, an app has
/// several of these on screen at once and `on_drag_move` filters by type alone
/// — without the id every bar in the window would answer one thumb's gesture.
#[derive(Clone)]
pub struct ScrollbarDrag(pub SharedString);

/// Where in the thumb a drag was grabbed.
///
/// Shaped like gpui's `ScrollHandle` — an `Rc` cell the view holds one field of
/// and the bar clones — for the same reason: both mutate through `&self`, so
/// the bar carries its whole gesture without the view wiring a single listener.
/// Without it the thumb would jump its middle to the pointer on every press.
#[derive(Clone, Default)]
pub struct ScrollbarState(Rc<Cell<Option<Pixels>>>);

impl ScrollbarState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a thumb drag is in flight.
    pub fn dragging(&self) -> bool {
        self.0.get().is_some()
    }
}

/// The bar: an overlay strip along the right edge of whatever it is laid over,
/// showing nothing at all when the content fits.
///
/// Overlay rather than a gutter, so a bar arriving or leaving never reflows the
/// content beneath it.
///
/// Its geometry comes from the handle as the *last* frame left it, which is all
/// a render pass can see; the canvas at the end asks for one more frame when
/// layout disagrees, so the bar is right on the frame after it first appears
/// rather than whenever something else happens to repaint.
///
/// No `&Theme`, unlike most of this crate — a scrollbar is a neutral overlay
/// rather than a toned surface, so the thumb is [`ink`], which already follows
/// the appearance on its own. A parameter it ignored would be worse than none.
pub fn scrollbar(
    id: impl Into<SharedString>,
    handle: &ScrollHandle,
    state: &ScrollbarState,
) -> gpui::AnyElement {
    let id = id.into();
    let viewport = handle.bounds().size.height;
    let max_offset = handle.max_offset().y;
    let Some(range) = thumb(viewport, max_offset, handle.offset().y, MIN_THUMB) else {
        return Empty.into_any_element();
    };
    let size = range.end - range.start;
    let dragging = state.dragging();

    let track_id = id.clone();
    let drag_handle = handle.clone();
    let drag_state = state.clone();
    let release_state = state.clone();
    let released = move |_: &gpui::MouseUpEvent, _: &mut Window, _: &mut App| {
        release_state.0.set(None);
    };

    div()
        .id(SharedString::from(format!("{id}-track")))
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .w(px(TRACK))
        .flex()
        .justify_center()
        .on_drag_move(move |event: &DragMoveEvent<ScrollbarDrag>, window, cx| {
            // Another bar's thumb: `on_drag_move` filters by payload type, and
            // every bar in the window shares this one.
            if event.drag(cx).0 != track_id {
                return;
            }
            let viewport = drag_handle.bounds().size.height;
            let max_offset = drag_handle.max_offset().y;
            let Some(range) = thumb(viewport, max_offset, drag_handle.offset().y, MIN_THUMB) else {
                return;
            };
            let size = range.end - range.start;
            let pointer = event.event.position.y - event.bounds.top();
            // First move of this drag: the offset has not shifted yet, so the
            // thumb is still where the press landed on it and the grab is
            // simply the difference. Held for the rest of the gesture — read it
            // again later and it would answer "wherever the pointer is now",
            // which is a thumb that never moves.
            let grab = drag_state.0.get().unwrap_or_else(|| {
                let grab = (pointer - range.start).clamp(px(0.0), size);
                drag_state.0.set(Some(grab));
                grab
            });
            let offset = offset_for_thumb(pointer - grab, viewport, max_offset, size);
            drag_handle.set_offset(point(drag_handle.offset().x, offset));
            window.refresh();
        })
        // Both, because a release can land anywhere on screen; a grab left set
        // would make the next press continue the last gesture.
        .on_mouse_up(MouseButton::Left, released.clone())
        .on_mouse_up_out(MouseButton::Left, released)
        .child(
            div()
                .id(SharedString::from(format!("{id}-thumb")))
                .absolute()
                .top(range.start)
                .h(size)
                .w(px(THUMB))
                .rounded_full()
                .bg(if dragging { ink(0.38) } else { ink(0.2) })
                .hover(|s| s.bg(ink(0.32)))
                .on_drag(ScrollbarDrag(id.clone()), |_, _, _, cx| cx.new(|_| Empty)),
        )
        .child(
            canvas(
                move |bounds, window, _| {
                    // Laid out taller or shorter than the geometry above was
                    // computed from: that geometry came from last frame's
                    // handle. Ask for the frame that will paint it right.
                    // Self-limiting — once they agree, nothing is requested.
                    if (bounds.size.height - viewport).abs() > px(0.5) {
                        window.refresh();
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 400px window onto 1600px of content: 1200 of overflow.
    const VIEWPORT: Pixels = px(400.0);
    const MAX: Pixels = px(1200.0);

    #[test]
    fn a_thumb_is_as_long_as_the_visible_share() {
        // 400 of 1600 visible — a quarter of the track.
        let range = thumb(VIEWPORT, MAX, px(0.0), MIN_THUMB).expect("scrollable");
        assert_eq!(range.start, px(0.0));
        assert_eq!(range.end - range.start, px(100.0));
    }

    #[test]
    fn the_thumb_travels_the_track_and_no_further() {
        let size = px(100.0);
        // Scrolled to the end: the thumb's END meets the track's, so its top
        // stops a thumb's length short.
        let range = thumb(VIEWPORT, MAX, -MAX, MIN_THUMB).expect("scrollable");
        assert_eq!(range.start, VIEWPORT - size);
        assert_eq!(range.end, VIEWPORT);
        // Halfway.
        let range = thumb(VIEWPORT, MAX, -(MAX / 2.0), MIN_THUMB).expect("scrollable");
        assert_eq!(range.start, (VIEWPORT - size) / 2.0);
    }

    #[test]
    fn an_overshot_offset_is_clamped_rather_than_escaping() {
        // A wheel can push past either end before the container clamps.
        let past = thumb(VIEWPORT, MAX, px(-9000.0), MIN_THUMB).expect("scrollable");
        assert_eq!(
            past,
            thumb(VIEWPORT, MAX, -MAX, MIN_THUMB).expect("scrollable")
        );
        let before = thumb(VIEWPORT, MAX, px(500.0), MIN_THUMB).expect("scrollable");
        assert_eq!(before.start, px(0.0));
    }

    #[test]
    fn nothing_to_scroll_means_no_bar() {
        assert!(thumb(VIEWPORT, px(0.0), px(0.0), MIN_THUMB).is_none());
        // The frame before layout has run.
        assert!(thumb(px(0.0), MAX, px(0.0), MIN_THUMB).is_none());
        // A viewport shorter than the minimum thumb: all thumb, no travel.
        assert!(thumb(px(20.0), MAX, px(0.0), MIN_THUMB).is_none());
    }

    #[test]
    fn dragging_the_thumb_back_gives_the_offset_that_drew_it() {
        // The round trip both directions have to agree on, or the thumb slides
        // out from under the pointer.
        for offset in [px(0.0), px(-1.0), px(-600.0), -MAX] {
            let range = thumb(VIEWPORT, MAX, offset, MIN_THUMB).expect("scrollable");
            let size = range.end - range.start;
            assert_eq!(
                offset_for_thumb(range.start, VIEWPORT, MAX, size),
                offset,
                "round trip at {offset:?}"
            );
        }
    }

    #[test]
    fn a_clamped_thumb_still_reaches_the_end() {
        // 400px onto 100_000: the geometric thumb would be 1.6px, so the
        // minimum takes over — and the round trip has to survive that, since
        // travel is now measured against the clamped size and not the real one.
        let max = px(99_600.0);
        let range = thumb(VIEWPORT, max, -max, MIN_THUMB).expect("scrollable");
        assert_eq!(range.end - range.start, MIN_THUMB);
        assert_eq!(range.end, VIEWPORT);
        assert_eq!(
            offset_for_thumb(range.start, VIEWPORT, max, MIN_THUMB),
            -max
        );
    }

    #[test]
    fn a_drag_past_either_end_stops_there() {
        assert_eq!(
            offset_for_thumb(px(10_000.0), VIEWPORT, MAX, px(100.0)),
            -MAX
        );
        assert_eq!(
            offset_for_thumb(px(-10_000.0), VIEWPORT, MAX, px(100.0)),
            px(0.0)
        );
        // Degenerate: a thumb filling its track has nowhere to go.
        assert_eq!(offset_for_thumb(px(50.0), VIEWPORT, MAX, VIEWPORT), px(0.0));
    }
}
