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
//!
//! [`transient`] is the same bar, shown only while its content moves.

use std::{cell::Cell, ops::Range, rc::Rc, time::Duration};

use gpui::{
    Animation, AnimationExt, App, DragMoveEvent, ElementId, Empty, MouseButton, Pixels,
    ScrollHandle, SharedString, Window, canvas, div, point, prelude::*, px,
};

use motion::Painter;
use theme::ink;

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
#[derive(Clone)]
pub struct ScrollbarState {
    grab: Rc<Cell<Option<Pixels>>>,
    /// A drag runs in event-dispatch context, where the window cannot resolve
    /// which view is asking — so the bar carries its own.
    painter: Painter,
}

impl ScrollbarState {
    pub fn new(painter: Painter) -> Self {
        Self {
            grab: Rc::new(Cell::new(None)),
            painter,
        }
    }

    /// Whether a thumb drag is in flight.
    pub fn dragging(&self) -> bool {
        self.grab.get().is_some()
    }

    /// One drag move of the thumb: filter the gesture to this bar's track, then
    /// translate the pointer into a scroll offset.
    fn drag(
        &self,
        track_id: &SharedString,
        handle: &ScrollHandle,
        event: &DragMoveEvent<ScrollbarDrag>,
        cx: &mut App,
    ) {
        // Another bar's thumb: `on_drag_move` filters by payload type, and
        // every bar in the window shares this one.
        if event.drag(cx).0 != *track_id {
            return;
        }
        let viewport = handle.bounds().size.height;
        let max_offset = handle.max_offset().y;
        let Some(range) = thumb(viewport, max_offset, handle.offset().y, MIN_THUMB) else {
            return;
        };
        let size = range.end - range.start;
        let pointer = event.event.position.y - event.bounds.top();
        // First move of this drag: the offset has not shifted yet, so the
        // thumb is still where the press landed on it and the grab is
        // simply the difference. Held for the rest of the gesture — read it
        // again later and it would answer "wherever the pointer is now",
        // which is a thumb that never moves.
        let grab = self.grab.get().unwrap_or_else(|| {
            let grab = (pointer - range.start).clamp(px(0.0), size);
            self.grab.set(Some(grab));
            grab
        });
        let offset = offset_for_thumb(pointer - grab, viewport, max_offset, size);
        handle.set_offset(point(handle.offset().x, offset));
        self.painter.notify(cx);
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
        release_state.grab.set(None);
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
        .on_drag_move(move |event, _, cx| {
            drag_state.drag(&track_id, &drag_handle, event, cx);
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
                        window.request_animation_frame();
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
// Transient — the same bar, only while the content moves
// ---------------------------------------------------------------------------

/// How long the thumb stays after the last scroll before fading — the idle
/// window *is* the fade, because the fork's `Animation` has no delay.
pub const TRANSIENT_IDLE: Duration = Duration::from_millis(1000);

/// The show-and-fade state behind [`transient`]: the last frame's scroll
/// state (a change is activity), a generation counter (a fresh animation id
/// restarts the fade — `AnimationElement` pins its clock to the id it first
/// laid out with), and the hover flag that holds the thumb up while the
/// pointer is on the strip.
#[derive(Clone)]
pub struct TransientState {
    cell: Rc<Cell<(Pixels, Pixels, u64, bool)>>,
    bar: ScrollbarState,
}

impl TransientState {
    pub fn new(painter: Painter) -> Self {
        Self {
            cell: Rc::new(Cell::new(Default::default())),
            bar: ScrollbarState::new(painter),
        }
    }
}

/// The same bar as [`scrollbar`], but it only earns its place while the
/// content moves: activity raises the thumb, and it fades out over
/// [`TRANSIENT_IDLE`] once the scrolling stops. Hovering the strip or
/// dragging the thumb holds it up. With `reduce_motion` there is nothing to
/// animate, so it renders as the always-on bar.
pub fn transient(
    id: impl Into<SharedString>,
    handle: &ScrollHandle,
    state: &TransientState,
    reduce_motion: bool,
) -> gpui::AnyElement {
    let id = id.into();
    let viewport = handle.bounds().size.height;
    let max_offset = handle.max_offset().y;
    let Some(range) = thumb(viewport, max_offset, handle.offset().y, MIN_THUMB) else {
        return Empty.into_any_element();
    };
    let size = range.end - range.start;

    // Any change in the scroll state is activity: bump the generation so the
    // fade restarts under a fresh animation id. The half-pixel slack keeps a
    // sub-pixel layout jitter from re-showing a settled bar.
    let mut cell = state.cell.get();
    if (handle.offset().y - cell.0).abs() > px(0.5) || (max_offset - cell.1).abs() > px(0.5) {
        cell.0 = handle.offset().y;
        cell.1 = max_offset;
        cell.2 += 1;
        state.cell.set(cell);
    }
    let generation = cell.2;
    let dragging = state.bar.dragging();

    let track_id = id.clone();
    let drag_handle = handle.clone();
    let drag_state = state.clone();
    let release_state = state.clone();
    let released = move |_: &gpui::MouseUpEvent, _: &mut Window, _: &mut App| {
        release_state.bar.grab.set(None);
    };

    let track = div()
        .id(SharedString::from(format!("{id}-track")))
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .w(px(TRACK))
        .flex()
        .justify_center()
        .on_drag_move(move |event, _, cx| {
            drag_state.bar.drag(&track_id, &drag_handle, event, cx);
        })
        // Both, because a release can land anywhere on screen; a grab left set
        // would make the next press continue the last gesture.
        .on_mouse_up(MouseButton::Left, released.clone())
        .on_mouse_up_out(MouseButton::Left, released)
        .map(|track| {
            if reduce_motion {
                track
            } else {
                // The thumb's stand-in at rest: hovering the strip raises it,
                // and leaving starts its fade.
                let hover_state = state.clone();
                let hover_painter = state.bar.painter;
                track.on_hover(move |hovered: &bool, _, cx: &mut App| {
                    let mut cell = hover_state.cell.get();
                    if cell.3 == *hovered {
                        return;
                    }
                    cell.3 = *hovered;
                    cell.2 += 1;
                    hover_state.cell.set(cell);
                    hover_painter.notify(cx);
                })
            }
        });

    let thumb = div()
        .id(SharedString::from(format!("{id}-thumb")))
        .absolute()
        .top(range.start)
        .h(size)
        .w(px(THUMB))
        .rounded_full()
        .bg(if dragging { ink(0.38) } else { ink(0.2) })
        .hover(|s| s.bg(ink(0.32)))
        .on_drag(ScrollbarDrag(id.clone()), |_, _, _, cx| cx.new(|_| Empty));

    let thumb: gpui::AnyElement = if reduce_motion {
        thumb.into_any_element()
    } else {
        // The thumb's presence is the animation: progress 0 is fully up, and
        // progress 1 — a full idle window later — is hidden, so no frame is
        // requested once the fade completes.
        let anim = state.clone();
        thumb
            .with_animation(
                ElementId::from(format!("{id}-fade-{generation}")),
                Animation::new(TRANSIENT_IDLE),
                move |el, p| {
                    let cell = anim.cell.get();
                    if cell.3 || anim.bar.dragging() {
                        el
                    } else if p < 1.0 {
                        el.opacity(1.0 - p)
                    } else {
                        el.hidden()
                    }
                },
            )
            .into_any_element()
    };

    track
        .child(thumb)
        .child(
            canvas(
                move |bounds, window, _| {
                    // Laid out taller or shorter than the geometry above was
                    // computed from: that geometry came from last frame's
                    // handle. Ask for the frame that will paint it right.
                    // Self-limiting — once they agree, nothing is requested.
                    if (bounds.size.height - viewport).abs() > px(0.5) {
                        window.request_animation_frame();
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
                window.request_animation_frame();
            }
            state.0.set((pinned, max_offset));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
    .into_any_element()
}
