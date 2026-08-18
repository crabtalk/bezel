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

// ---------------------------------------------------------------------------
// Follow — a view pinned to the bottom of content that grows under it
// ---------------------------------------------------------------------------

/// How close to the bottom still counts as following. A wheel lands on
/// fractional offsets and a re-layout can move the end by a hair; without slack
/// a view would unpin itself for a rounding error nobody asked for.
pub const FOLLOW_SLACK: Pixels = px(4.0);

/// Whether `offset` is at the end of the scrollable range, within `slack`.
///
/// Both of gpui's conventions bite here, so: `max_offset` is the *overflow* and
/// `offset` is **negative** going down, which makes the distance still to go
/// `max_offset - |offset|`. Content that fits is always "at the bottom" — there
/// is nowhere else to be, and answering `false` would unpin an empty log.
pub fn at_bottom(max_offset: Pixels, offset: Pixels, slack: Pixels) -> bool {
    if max_offset <= px(0.0) {
        return true;
    }
    let travelled = offset.clamp(-max_offset, px(0.0)).abs();
    max_offset - travelled <= slack
}

/// Whether a [`follow`] view is still pinned, and the overflow it last saw.
///
/// Shaped like [`ScrollbarState`] and for the same reason: it mutates through
/// `&self`, so the element carries the whole behaviour without the view wiring
/// a listener. Starts pinned — a transcript or a log opens on its newest line.
#[derive(Clone)]
pub struct FollowState(Rc<Cell<(bool, Pixels)>>);

impl Default for FollowState {
    fn default() -> Self {
        Self(Rc::new(Cell::new((true, px(0.0)))))
    }
}

impl FollowState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the view is following. An app shows its "jump to latest" affordance
    /// on `!following()`, which is the only reason this is public.
    pub fn following(&self) -> bool {
        self.0.get().0
    }

    /// Re-pin. What that "jump to latest" button calls; the next frame does the
    /// scrolling.
    pub fn follow(&self) {
        let (_, last) = self.0.get();
        self.0.set((true, last));
    }
}

/// Keep `handle` pinned to the bottom of its content while the user leaves it
/// there, and get out of the way the moment they scroll up.
///
/// Drop it in beside [`scrollbar`], over the same container:
///
/// ```ignore
/// div().relative()
///     .child(div().id("log").size_full().overflow_y_scroll().track_scroll(&self.scroll).child(rows))
///     .child(scroll::follow(&self.scroll, &self.follow))
///     .child(scroll::scrollbar("log-bar", &self.scroll, &self.bar))
/// ```
///
/// **Telling appended content from a user scroll is the whole problem**, and
/// neither is an event this can subscribe to — both surface as the same handle
/// reading differently than last frame. The overflow is what separates them: if
/// it changed, the content grew and the pin is left as the user last set it; if
/// it did not, the offset moved because the *user* moved it, and being at the
/// end is what re-pins. So scrolling up releases, and scrolling back down
/// re-attaches, with no gesture to hook.
///
/// The correction lands a frame late — the scrolling div was laid out with the
/// old offset before this runs — which is why it asks for that frame. At a
/// streaming cadence it is invisible, and it converges rather than spinning:
/// once pinned and at the end, nothing is requested.
pub fn follow(handle: &ScrollHandle, state: &FollowState) -> gpui::AnyElement {
    let handle = handle.clone();
    let state = state.clone();
    canvas(
        move |_, window, _| {
            let max_offset = handle.max_offset().y;
            let offset = handle.offset().y;
            let (was_pinned, last_max) = state.0.get();

            let pinned = if (max_offset - last_max).abs() > px(0.5) {
                was_pinned
            } else {
                at_bottom(max_offset, offset, FOLLOW_SLACK)
            };

            if pinned && (offset + max_offset).abs() > px(0.5) {
                handle.set_offset(point(handle.offset().x, -max_offset));
                window.refresh();
            }
            state.0.set((pinned, max_offset));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
    .into_any_element()
}
