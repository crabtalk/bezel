//! What gpui's own scroll handles leave to the app: a container that scrolls
//! the way it was asked to, a bar to show the position, and a rule for which
//! pane a wheel belongs to.
//!
//! [`pane`] is the container — an axis given at construction, the way
//! SwiftUI's `ScrollView` takes one, because gpui's is a style field that
//! defaults to unset and gets guessed at. Read its docs before reaching for
//! `div().overflow_y_scroll()`; the guess is a real bug and not a small one.
//!
//! gpui scrolls that pane perfectly well and draws nothing while it does, so a
//! bezel app has no way to show how far down it is. That is [`scrollbar`]: an
//! overlay the caller lays over its own pane, because a wrapper that swallowed
//! the content would have to re-implement layout for it. Nesting two panes is
//! [`claim_wheel`]'s business.
//!
//! ```ignore
//! div().relative()                                  // the bar is absolute in here
//!     .child(
//!         scroll::pane("pane", Axes::Vertical)
//!             .size_full()
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
//! [`Overlay`] manages its own state and supports either axis. Its default is
//! [`Visibility::Scrolling`]; [`set_visibility`] updates all default overlays,
//! including Markdown code blocks and tables. [`Viewport`] also owns the handle.

mod overlay;
pub use overlay::{Overlay, Viewport, Visibility, set_visibility, visibility};

use std::{cell::Cell, ops::Range, rc::Rc, time::Duration};

use gpui::{
    Animation, AnimationExt, App, Axis, Bounds, Div, DragMoveEvent, ElementId, Empty, MouseButton,
    Pixels, Point, ScrollHandle, SharedString, Stateful, Window, canvas, div, point, prelude::*,
    px,
};

use motion::Painter;
use theme::ink;
use web_time::Instant;

/// Shortest a thumb may get, however long the document — below this it stops
/// being something a pointer can catch.
pub const MIN_THUMB: Pixels = px(25.0);
/// Space between the overlay track and the viewport edges, along the axis the
/// bar runs.
const INSET: f32 = 4.0;
const BAR_INSET: Pixels = px(INSET);
/// Width of the strip the thumb sits in.
const TRACK: f32 = 10.0;
/// Room a bar is centred in across its axis when the caller reserves none.
const CHANNEL: f32 = 2.0 * INSET + TRACK;
/// Width of the thumb itself, centred in the track.
pub(crate) const THUMB: f32 = 4.0;
/// Length of one [`rail`] mark, and its thickness.
const MARK: f32 = 16.0;
const MARK_THICK: f32 = 2.0;
/// Between two marks. The track's width, so a rail and a bar on the same pane
/// are cut to one rhythm.
const MARK_GAP: f32 = TRACK;
/// How far the rail stands off the edge it is pinned to.
const RAIL_INSET: f32 = 12.0;
/// What a rail needs beside the content before it will paint at all.
const RAIL_ROOM: f32 = RAIL_INSET + MARK;

// ---------------------------------------------------------------------------
// Pane — a scroll container whose axis is an argument, not a modifier
// ---------------------------------------------------------------------------

/// Which way a [`pane`] scrolls. SwiftUI's `Axis.Set`, which gpui's [`Axis`]
/// has no spelling for: a pane that scrolls both ways is not one of two
/// directions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axes {
    Vertical,
    Horizontal,
    Both,
}

impl Axes {
    pub fn vertical(self) -> bool {
        matches!(self, Axes::Vertical | Axes::Both)
    }

    pub fn horizontal(self) -> bool {
        matches!(self, Axes::Horizontal | Axes::Both)
    }

    /// The gpui axis this is, where it is only one.
    pub fn axis(self) -> Option<Axis> {
        match self {
            Axes::Vertical => Some(Axis::Vertical),
            Axes::Horizontal => Some(Axis::Horizontal),
            Axes::Both => None,
        }
    }
}

/// A scroll container, with the axis as an argument rather than a modifier you
/// can forget.
///
/// ```ignore
/// scroll::pane("log", Axes::Vertical)
///     .size_full()
///     .track_scroll(&self.scroll)
///     .child(content)
/// ```
///
/// Returns an element for the caller to fill, the way [`crate::stack::row`]
/// does — it takes no children and lays nothing out, so the pane stays the
/// app's and only its scroll behaviour is decided here. The id is gpui's
/// requirement, not ours: a scroll container has state to track.
///
/// # Why this exists rather than `div().overflow_y_scroll()`
///
/// gpui makes scrollability a late-bound style field with no default, and then
/// has to guess what to do when a gesture's axis is not one the container
/// scrolls: it **remaps the delta onto whichever axis the container can
/// scroll**. A sideways swipe over a vertical list scrolls it down; a downward
/// swipe over a wide table pans it sideways. `restrict_scroll_to_axis` turns
/// that off, but it is opt-in per element, so every pane that forgets it is
/// wrong and nothing says so.
///
/// SwiftUI has no such case to guess at — `ScrollView(.vertical)` takes its
/// axis at construction, so there is no container whose axis is unstated. This
/// is that: ask for an axis, get a pane that answers only to it.
///
/// A horizontal pane also contains a sideways gesture ([`contain_sideways`]),
/// because the pane it is nested in usually belongs to a consumer and is not
/// ours to restrict.
///
/// `Axes::Both` inherits gpui's dominant-axis lock — a diagonal gesture moves
/// one axis, not two. gpui exposes no builder for `allow_concurrent_scroll`.
pub fn pane(id: impl Into<ElementId>, axes: Axes) -> Stateful<Div> {
    scrolls(div().id(id), axes)
}

/// [`pane`], keeping the wheel it can act on: the pane a consumer nests inside
/// another and never wires a handle to.
///
/// [`claim_wheel`] asks the caller for a [`ScrollHandle`] and a [`ClaimState`],
/// because the caller usually has the handle already — it is scrolling the pane
/// from elsewhere. A bounded box inside someone else's page has neither, and a
/// pane that is only ever read by the wheel that moves it should not make a
/// consumer hold two fields to stop it dragging the page behind it. Both live
/// in keyed element state here, so the pane is still one call.
///
/// ```ignore
/// scroll::claiming_pane("output", Axes::Vertical, window, cx).child(text)
/// ```
///
/// The chaining is [`claim_wheel`]'s: at its ends the pane hands the wheel back
/// to the page.
pub fn claiming_pane(
    id: impl Into<ElementId>,
    axes: Axes,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let id = id.into();
    let held = window.use_keyed_state(id.clone(), cx, |_, _| Claiming::default());
    let (handle, state) = {
        let held = held.read(cx);
        (held.handle.clone(), held.state.clone())
    };
    claim_wheel(pane(id, axes).track_scroll(&handle), &handle, axes, &state)
}

/// What [`claiming_pane`] keeps between frames: the handle it reads its own
/// travel off, and where that travel stood before the wheel being dispatched.
#[derive(Default)]
struct Claiming {
    handle: ScrollHandle,
    state: ClaimState,
}

/// [`pane`]'s answer applied to an element that already exists — a container
/// that scrolls only at some widths, or one another builder handed back.
///
/// ```ignore
/// strip.when(compact, |strip| scroll::scrolls(strip, Axes::Horizontal))
/// ```
pub fn scrolls<E: gpui::StatefulInteractiveElement>(el: E, axes: Axes) -> E {
    let el = match axes {
        Axes::Vertical => el.overflow_y_scroll(),
        Axes::Horizontal => el.overflow_x_scroll(),
        Axes::Both => el.overflow_scroll(),
    }
    .restrict_scroll_to_axis();
    match axes.horizontal() {
        true => contain_sideways(el),
        false => el,
    }
}

/// Keep a sideways gesture inside the pane it started in.
///
/// The other half of [`pane`], and the half [`Axes::Vertical`] does not want:
/// a vertical pane at its end should hand the wheel to the page behind it
/// ([`claim_wheel`] is that chaining), but a sideways gesture reaching a
/// vertical ancestor is never right — unless that ancestor is restricted too,
/// it will remap the delta and scroll down.
///
/// Applied by [`pane`] for the axes that need it. Public because a consumer
/// wrapping bezel's content in a scroller of its own has the same problem and
/// the same fix.
///
/// Registered before the element's own handler and so run after it — gpui
/// bubbles the list backwards — which is why the pane has already moved by the
/// time the event stops here.
pub fn contain_sideways<E: gpui::InteractiveElement>(el: E) -> E {
    contain_wheel(el, Axes::Horizontal)
}

/// Keep every wheel inside the pane it landed on — `overscroll-behavior:
/// contain`, where [`claim_wheel`] is the chaining kind.
///
/// For a box with a cap on it, where the content is a program's output rather
/// than a document: it is a window onto something, and a wheel over a window
/// belongs to what is inside it. Chaining asks the pane to prove it moved,
/// which it reads off a handle carrying the previous frame's layout — under a
/// pane whose content is still arriving that reads as "did not move", and the
/// page takes the wheel while the box is still scrolling (user report,
/// DEV-13).
///
/// The page is still reachable: move the pointer off the box.
pub fn contain_wheel<E: gpui::InteractiveElement>(el: E, axes: Axes) -> E {
    el.on_scroll_wheel(move |event, window, cx| {
        let delta = event.delta.pixel_delta(window.line_height());
        // The dominant axis, not "any horizontal component": a trackpad puts a
        // little of both into every gesture, and a mostly-vertical one still
        // belongs to the page.
        let sideways = delta.x.abs() > delta.y.abs();
        if (sideways && axes.horizontal()) || (!sideways && axes.vertical()) {
            cx.stop_propagation();
        }
    })
}

/// The strip a thumb sits in, placed along `axis` and named for `id`.
///
/// A press on the bar belongs to the bar. Hitboxes in gpui are paint-order
/// only, so without `block_mouse_except_scroll` the content under the strip
/// takes the press as well; the wheel still passes, which is what a bar laid
/// over a pane has to let through.
/// Whether the pointer rests on a bar's track, which holds a transient bar up.
///
/// gpui keeps the pointer's last position when it leaves the window, and a
/// track under that position reads as hovered again on the next frame. `out`
/// is set when the pointer leaves the window across the track and cleared by
/// the next move over it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Hover {
    over: bool,
    out: bool,
}

impl Hover {
    pub(crate) fn held(self) -> bool {
        self.over && !self.out
    }
}

type SetHover = Rc<dyn Fn(Hover, &mut Window, &mut App)>;

/// Keep `hover` for `track`, calling `changed` when it changes.
pub(crate) fn hover_track<E: StatefulInteractiveElement>(
    track: E,
    hover: Rc<Cell<Hover>>,
    changed: impl Fn(&mut Window, &mut App) + 'static,
) -> E {
    let set: SetHover = {
        let hover = hover.clone();
        Rc::new(move |next, window, cx| {
            if next != hover.get() {
                hover.set(next);
                changed(window, cx);
            }
        })
    };
    let (over, exit, moved) = (hover.clone(), hover.clone(), hover);
    let (set_exit, set_moved) = (set.clone(), set.clone());
    track
        .on_hover(move |hovered, window, cx| {
            let over = Hover {
                over: *hovered,
                ..over.get()
            };
            set(over, window, cx)
        })
        .on_mouse_exit(move |_, window, cx| {
            let out = Hover {
                out: true,
                ..exit.get()
            };
            set_exit(out, window, cx)
        })
        .on_mouse_move(move |_, window, cx| {
            let back = Hover {
                out: false,
                ..moved.get()
            };
            set_moved(back, window, cx)
        })
}

fn track(id: &SharedString, place: Place, axis: Axis) -> Stateful<Div> {
    let debug_id = id.clone();
    let el = div()
        .debug_selector(move || format!("{debug_id}-track"))
        .id(SharedString::from(format!("{id}-track")))
        .block_mouse_except_scroll()
        .absolute()
        .flex();
    match axis {
        Axis::Vertical => el
            .top(BAR_INSET)
            .right(place.near())
            .bottom(BAR_INSET + place.end)
            .w(px(TRACK))
            .justify_center(),
        Axis::Horizontal => el
            .left(BAR_INSET)
            .right(BAR_INSET + place.end)
            .bottom(place.near())
            .h(px(TRACK))
            .items_center(),
    }
}

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

pub(crate) fn thumb_in_track(
    viewport: Pixels,
    max_offset: Pixels,
    offset: Pixels,
    track: Pixels,
) -> Option<Range<Pixels>> {
    if track <= px(0.) {
        return None;
    }
    let scale = track / viewport;
    let range = thumb(viewport, max_offset, offset, MIN_THUMB / scale)?;
    Some(range.start * scale..range.end * scale)
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

    fn begin(&self, handle: &ScrollHandle, event: &gpui::MouseDownEvent, end_inset: Pixels) {
        let viewport = handle.bounds().size.height;
        if let Some(range) = thumb_in_track(
            viewport,
            handle.max_offset().y,
            handle.offset().y,
            viewport - 2. * BAR_INSET - end_inset,
        ) {
            self.grab.set(Some(
                (event.position.y - handle.bounds().top() - BAR_INSET - range.start)
                    .clamp(px(0.), range.end - range.start),
            ));
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
        end_inset: Pixels,
        cx: &mut App,
    ) {
        // Another bar's thumb: `on_drag_move` filters by payload type, and
        // every bar in the window shares this one.
        if event.drag(cx).0 != *track_id {
            return;
        }
        let viewport = handle.bounds().size.height;
        let max_offset = handle.max_offset().y;
        let Some(range) = thumb_in_track(
            viewport,
            max_offset,
            handle.offset().y,
            viewport - 2. * BAR_INSET - end_inset,
        ) else {
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
        let offset = offset_for_thumb(
            pointer - grab,
            viewport - 2. * BAR_INSET - end_inset,
            max_offset,
            size,
        );
        handle.set_offset(point(handle.offset().x, offset));
        self.painter.notify(cx);
    }
}

/// Where a bar sits in the pane it reports on. [`Overlay`] builds one; the free
/// bars take the default.
#[derive(Clone, Copy)]
struct Place {
    /// Shortens the track at its far end.
    end: Pixels,
    /// Room reserved across the axis, which the track is centred in.
    channel: Pixels,
}

impl Default for Place {
    fn default() -> Self {
        Self {
            end: px(0.),
            channel: px(CHANNEL),
        }
    }
}

impl Place {
    /// Gap between the near edge of the pane and the near side of the track.
    fn near(self) -> Pixels {
        ((self.channel - px(TRACK)) * 0.5).max(px(0.))
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
    scrollbar_placed(id.into(), handle, state, Place::default())
}

fn scrollbar_placed(
    id: SharedString,
    handle: &ScrollHandle,
    state: &ScrollbarState,
    place: Place,
) -> gpui::AnyElement {
    let end_inset = place.end;
    let viewport = handle.bounds().size.height;
    let max_offset = handle.max_offset().y;
    let Some(range) = thumb_in_track(
        viewport,
        max_offset,
        handle.offset().y,
        viewport - 2. * BAR_INSET - end_inset,
    ) else {
        return Empty.into_any_element();
    };
    let size = range.end - range.start;
    let dragging = state.dragging();

    let track_id = id.clone();
    let drag_handle = handle.clone();
    let drag_state = state.clone();
    let release_state = state.clone();
    let press_state = state.clone();
    let press_handle = handle.clone();
    let released = move |_: &gpui::MouseUpEvent, _: &mut Window, _: &mut App| {
        release_state.grab.set(None);
    };

    let thumb_debug_id = id.clone();
    track(&id, place, Axis::Vertical)
        .on_drag_move(move |event, _, cx| {
            drag_state.drag(&track_id, &drag_handle, event, end_inset, cx);
        })
        // Both, because a release can land anywhere on screen; a grab left set
        // would make the next press continue the last gesture.
        .on_mouse_up(MouseButton::Left, released.clone())
        .on_mouse_up_out(MouseButton::Left, released)
        .child(
            div()
                .debug_selector(move || format!("{thumb_debug_id}-thumb"))
                .id(SharedString::from(format!("{id}-thumb")))
                .absolute()
                .top(range.start)
                .h(size)
                .w(px(THUMB))
                .rounded_full()
                .bg(if dragging { ink(0.38) } else { ink(0.2) })
                .hover(|s| s.bg(ink(0.32)))
                .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                    press_state.begin(&press_handle, event, end_inset);
                    press_state.painter.notify(cx);
                })
                .on_drag(ScrollbarDrag(id.clone()), |_, _, _, cx| cx.new(|_| Empty)),
        )
        .child(
            canvas(
                move |bounds, window, _| {
                    // Laid out taller or shorter than the geometry above was
                    // computed from: that geometry came from last frame's
                    // handle. Ask for the frame that will paint it right.
                    // Self-limiting — once they agree, nothing is requested.
                    if (bounds.size.height + 2. * BAR_INSET + end_inset - viewport).abs() > px(0.5)
                    {
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

/// Whether `room` beside the content is enough for a rail to paint in. A
/// hand-rolled rail asks this to land on the same floor as [`rail`].
pub fn rail_fits(room: Pixels) -> bool {
    room >= px(RAIL_ROOM)
}

/// A mark per item, the one at the top of the viewport lit — for a pane whose
/// content comes in countable pieces (a transcript's turns) rather than as one
/// continuous document, where how far down you are matters less than which
/// piece you are on. A press jumps to that piece.
///
/// `count` addresses the **direct children** of the `track_scroll` element,
/// which is what gpui indexes: a pane whose pieces sit nested inside a wrapper
/// reports one child, and every mark would scroll to the same place.
///
/// Absolute, so the caller's container holds the position: pin it with
/// `.relative()` on whichever box the rail belongs to the edge of.
///
/// `room` is the clear space beside the content, which only the caller can
/// measure; under what [`rail_fits`] accepts the rail paints nothing.
pub fn rail(
    id: impl Into<SharedString>,
    handle: &ScrollHandle,
    count: usize,
    room: Pixels,
) -> gpui::AnyElement {
    if count == 0 || !rail_fits(room) {
        return Empty.into_any_element();
    }
    let id = id.into();
    let at = handle.top_item();
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(RAIL_INSET))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(MARK_GAP))
        .overflow_hidden()
        .children((0..count).map(|ix| {
            let handle = handle.clone();
            div()
                .id(SharedString::from(format!("{id}-{ix}")))
                .w(px(MARK))
                .h(px(MARK_THICK))
                .rounded_full()
                .bg(if ix == at { ink(0.6) } else { ink(0.2) })
                .cursor_pointer()
                .hover(|mark| mark.bg(ink(0.32)))
                .on_click(move |_, window, _| {
                    handle.scroll_to_item(ix);
                    window.refresh();
                })
        }))
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
    cell: Rc<Cell<(Pixels, Pixels, u64)>>,
    hover: Rc<Cell<Hover>>,
    bar: ScrollbarState,
}

impl TransientState {
    pub fn new(painter: Painter) -> Self {
        Self {
            cell: Rc::new(Cell::new(Default::default())),
            hover: Rc::default(),
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
    transient_placed(id.into(), handle, state, reduce_motion, Place::default())
}

fn transient_placed(
    id: SharedString,
    handle: &ScrollHandle,
    state: &TransientState,
    reduce_motion: bool,
    place: Place,
) -> gpui::AnyElement {
    let end_inset = place.end;
    let viewport = handle.bounds().size.height;
    let max_offset = handle.max_offset().y;
    let Some(range) = thumb_in_track(
        viewport,
        max_offset,
        handle.offset().y,
        viewport - 2. * BAR_INSET - end_inset,
    ) else {
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
    let press_state = state.clone();
    let press_handle = handle.clone();
    let released = move |_: &gpui::MouseUpEvent, _: &mut Window, _: &mut App| {
        release_state.bar.grab.set(None);
    };

    let thumb_debug_id = id.clone();
    let track = track(&id, place, Axis::Vertical)
        .on_drag_move(move |event, _, cx| {
            drag_state
                .bar
                .drag(&track_id, &drag_handle, event, end_inset, cx);
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
                hover_track(track, state.hover.clone(), move |_, cx| {
                    let mut cell = hover_state.cell.get();
                    cell.2 += 1;
                    hover_state.cell.set(cell);
                    hover_painter.notify(cx);
                })
            }
        });

    let thumb = div()
        .debug_selector(move || format!("{thumb_debug_id}-thumb"))
        .id(SharedString::from(format!("{id}-thumb")))
        .absolute()
        .top(range.start)
        .h(size)
        .w(px(THUMB))
        .rounded_full()
        .bg(if dragging { ink(0.38) } else { ink(0.2) })
        .hover(|s| s.bg(ink(0.32)))
        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
            press_state.bar.begin(&press_handle, event, end_inset);
            press_state.bar.painter.notify(cx);
        })
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
                    if anim.hover.get().held() || anim.bar.dragging() {
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
                    if (bounds.size.height + 2. * BAR_INSET + end_inset - viewport).abs() > px(0.5)
                    {
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
// Nesting — which pane a wheel belongs to
// ---------------------------------------------------------------------------

/// Where a [`claim_wheel`] pane was before the wheel that is being dispatched.
///
/// Shaped like [`ScrollbarState`] and [`FollowState`], and owned by the view
/// for the same reason: an element rebuilt every render cannot remember
/// anything, and this has to outlive the frame it was written in.
#[derive(Clone)]
pub struct ClaimState(Rc<Cell<Point<Pixels>>>);

impl Default for ClaimState {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaimState {
    pub fn new() -> Self {
        Self(Rc::new(Cell::new(point(px(0.0), px(0.0)))))
    }
}

/// Let a pane keep the wheel it can act on, instead of passing it to the pane
/// behind as well.
///
/// gpui's scroll listener neither stops propagation nor asks whether an
/// ancestor scrolls too, so a wheel over a nested pane moves *both* — an output
/// box inside a transcript scrolls itself and drags the transcript with it
/// (user report). Every scrolling ancestor under the pointer does this, so the
/// deeper the nesting the further the page jumps.
///
/// The question it asks is whether the pane *moved*, not whether it had room
/// to. This listener runs after the element's own — gpui registers that one
/// later and the bubble phase runs the list backwards — so `handle` already
/// holds the post-scroll offset, and `state` is where it stood before. "Had
/// room" is the same answer one notch too late, and gets the notch that lands
/// exactly on the end wrong: the pane finishes its travel *and* the page jumps
/// a full notch behind it.
///
/// Chained at the ends, not sealed: a pane with nowhere left to go hands the
/// wheel to the page, so reaching the end of a short inner list does not strand
/// it there and make the pointer move. A pane whose content fits never claims
/// anything, for the same reason. For `overscroll-behavior: contain` — a pane
/// that never lets a wheel past — stop unconditionally instead.
///
/// ```ignore
/// scroll::claim_wheel(
///     scroll::pane("output", Axes::Vertical).track_scroll(&self.scroll),
///     &self.scroll,
///     Axes::Vertical,
///     &self.claim,
/// )
/// ```
///
/// `axes` is what the pane scrolls, and must be what [`pane`] was given: an
/// axis left out here is one whose movement goes unnoticed, so the wheel is
/// handed on and the ancestor moves too.
pub fn claim_wheel<E: gpui::StatefulInteractiveElement>(
    el: E,
    handle: &ScrollHandle,
    axes: Axes,
    state: &ClaimState,
) -> E {
    let handle = handle.clone();
    let state = state.0.clone();
    // Where the pane stands as the frame is built, which is where it stands
    // before anything this frame's listeners are handed. The listener writes it
    // too: a wheel the pane could not act on produces no `notify` and so no
    // render, and the reading below has to stay true across that gap.
    state.set(travel(&handle, axes));
    el.on_scroll_wheel(move |_, _, cx| {
        let now = travel(&handle, axes);
        if now != state.get() {
            state.set(now);
            cx.stop_propagation();
        }
    })
}

/// How far `handle` has visibly travelled along each of `axes`. Negative, as
/// gpui counts it; an axis the pane does not scroll reads zero forever, so it
/// can never be mistaken for movement.
///
/// Clamped here because gpui's scroll listener is not: it adds the raw delta
/// and leaves the clamp to the next `paint`, so a pane held at its end keeps
/// accumulating offset it will never show. Compared raw, every notch past the
/// end reads as movement and the pane never lets go of the wheel.
fn travel(handle: &ScrollHandle, axes: Axes) -> Point<Pixels> {
    let (offset, max) = (handle.offset(), handle.max_offset());
    let seen = |offset: Pixels, max: Pixels| offset.clamp(-max.max(px(0.0)), px(0.0));
    point(
        match axes.horizontal() {
            true => seen(offset.x, max.x),
            false => px(0.0),
        },
        match axes.vertical() {
            true => seen(offset.y, max.y),
            false => px(0.0),
        },
    )
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
///     .child(scroll::pane("log", Axes::Vertical).size_full().track_scroll(&self.scroll).child(rows))
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

// ---------------------------------------------------------------------------
// Drift — a pane that keeps moving while a drag is held at its edge
// ---------------------------------------------------------------------------

/// How close to an edge a held drag starts the pane moving. Read off
/// `../desktop`'s board (2026-05): a third of a column, wide enough to reach
/// while aiming at a card and narrow enough to leave the middle still.
pub const DRIFT_EDGE: Pixels = px(96.0);

/// How fast a pane travels with the pointer at the very edge, in pixels a
/// second. The same board's 22px per frame, said in a unit that does not
/// double on a 120Hz display.
pub const DRIFT_SPEED: f32 = 1320.0;

/// The most one frame may travel, however late it ran. A frame dropped while
/// the pointer rests at an edge costs a pause, not a jump to the far end.
const DRIFT_STEP: Duration = Duration::from_millis(50);

/// How far a pane should travel in a second, for a pointer at `pointer`
/// between edges `start` and `end`.
///
/// Signed the way gpui's offset is: positive travels back toward the start,
/// because the offset goes negative as a pane scrolls on. Zero everywhere but
/// within [`DRIFT_EDGE`] of an edge, where it ramps with proximity — easing
/// toward the edge eases the scroll — and holds at full speed once the pointer
/// is past it, so a card carried off the side of a board keeps it coming
/// instead of stopping dead at the boundary.
///
/// The ramp is capped at half the pane, so one narrower than two edges has a
/// still middle rather than a midpoint where the direction flips at half speed.
pub fn drift_velocity(pointer: Pixels, start: Pixels, end: Pixels) -> f32 {
    let edge = DRIFT_EDGE.as_f32().min((end - start).as_f32() / 2.0);
    if edge <= 0.0 {
        return 0.0;
    }
    let (from_start, from_end) = ((pointer - start).as_f32(), (end - pointer).as_f32());
    // The nearer edge, which is what decides the direction — and what keeps
    // the two ramps from summing where they overlap.
    let near = from_start.min(from_end);
    if near >= edge {
        return 0.0;
    }
    let ramp = 1.0 - near.max(0.0) / edge;
    match from_start < from_end {
        true => DRIFT_SPEED * ramp,
        false => -DRIFT_SPEED * ramp,
    }
}

/// What lies past a drifting pane's edge, and so whether a pointer that has
/// crossed it is still aiming at the pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Beyond {
    /// Nothing the drag could be meant for — a board that fills the window.
    /// The pane holds at full speed for as long as the pointer is out there.
    Nothing,
    /// Another surface. The pane stops the moment the pointer leaves it,
    /// whichever edge it left by.
    Neighbour,
}

/// How fast a pane of `bounds` drifts with the pointer at `pointer`, one
/// [`drift_velocity`] per axis [`Axes`] names.
///
/// Across an axis the pointer must be within the pane: without that, every
/// lane of a board would drift together on a drag that is only near one of
/// them. Along it, `beyond` decides.
pub fn pane_velocity(
    bounds: Bounds<Pixels>,
    pointer: Point<Pixels>,
    axes: Axes,
    beyond: Beyond,
) -> Point<f32> {
    if beyond == Beyond::Neighbour && !bounds.contains(&pointer) {
        return point(0.0, 0.0);
    }
    let mut velocity = point(0.0, 0.0);
    if axes.horizontal() && (bounds.top()..=bounds.bottom()).contains(&pointer.y) {
        velocity.x = drift_velocity(pointer.x, bounds.left(), bounds.right());
    }
    if axes.vertical() && (bounds.left()..=bounds.right()).contains(&pointer.x) {
        velocity.y = drift_velocity(pointer.y, bounds.top(), bounds.bottom());
    }
    velocity
}

/// What one drifting pane remembers between frames.
#[derive(Clone, Copy, Default)]
struct Drift {
    /// Where the pointer was last seen carrying something this pane follows.
    aim: Option<Point<Pixels>>,
    /// When it last moved, and `None` whenever it is not moving — so a pane
    /// picking the gesture back up starts its clock rather than travelling the
    /// gap it stood still for.
    since: Option<Instant>,
}

/// Where a drift was last aimed, and when it last moved.
///
/// Shaped like [`ScrollbarState`] and owned by the view for the same reason:
/// the element is rebuilt every frame and cannot remember where the pointer
/// was. One per pane — two panes sharing one would take turns reading each
/// other's clock.
#[derive(Clone, Default)]
pub struct DriftState(Rc<Cell<Drift>>);

impl DriftState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Where the pointer is. Call it from the drag-move listener of the
    /// payload this pane should follow — that listener is typed, which is what
    /// keeps a board from drifting under somebody dragging a scrollbar thumb:
    ///
    /// ```ignore
    /// .on_drag_move(cx.listener(|this, event: &DragMoveEvent<CardDrag>, _, _| {
    ///     this.drift.aim(event.event.position);
    /// }))
    /// ```
    ///
    /// Aimed rather than continuous: gpui reports a drag only while it moves,
    /// and a pointer parked at an edge is the case the whole thing exists for.
    pub fn aim(&self, pointer: Point<Pixels>) {
        let drift = self.0.get();
        self.0.set(Drift {
            aim: Some(pointer),
            ..drift
        });
    }
}

/// Move `handle` while a drag is held near its edge, for as long as it is held
/// there.
///
/// Drop it in beside [`scrollbar`], over the same container, and feed it from
/// the drag-move listener — see [`DriftState::aim`]:
///
/// ```ignore
/// div().relative()
///     .child(scroll::pane("board", Axes::Horizontal).size_full().track_scroll(&self.scroll).child(lanes))
///     .child(scroll::drift(&self.scroll, &self.drift, Axes::Horizontal, Beyond::Nothing))
/// ```
///
/// Without it a board is only as wide as the window: a card cannot be carried
/// to a lane that is off screen, because reaching for one means letting go.
///
/// **The gesture ending is not an event this subscribes to.** gpui hands a
/// drop to whatever was under the pointer, and a release over nothing is not
/// delivered at all, so either would leave a pane drifting on a gesture that
/// is over. The drag going away is the signal instead, and it arrives however
/// the drag ended.
///
/// `beyond` is what the pane's edge gives onto, and decides whether a pointer
/// carried past it still drives the pane — see [`Beyond`] and
/// [`pane_velocity`].
///
/// Not motion in the [`motion`] sense and not reduced with it: nothing here
/// animates a property, the pane is being scrolled by a gesture the same way a
/// wheel scrolls it, and a reader who cannot reach the far lane has no gesture
/// left to make.
pub fn drift(
    handle: &ScrollHandle,
    state: &DriftState,
    axes: Axes,
    beyond: Beyond,
) -> gpui::AnyElement {
    let handle = handle.clone();
    let state = state.clone();
    canvas(
        move |_, window, cx: &mut App| {
            let drift = state.0.get();
            // Nothing aimed here, or the gesture that aimed it has ended.
            let Some(pointer) = drift.aim.filter(|_| cx.has_active_drag()) else {
                state.0.set(Drift::default());
                return;
            };

            let velocity = pane_velocity(handle.bounds(), pointer, axes, beyond);
            // Aimed here but nowhere near an edge, or gone off to a
            // neighbour. The clock stops with it, so a drag wandering back to
            // the edge a second later starts a fresh drift rather than
            // travelling the second it stood still.
            if velocity.x == 0.0 && velocity.y == 0.0 {
                state.0.set(Drift {
                    aim: Some(pointer),
                    since: None,
                });
                return;
            }

            let now = cx.background_executor().now();
            state.0.set(Drift {
                aim: Some(pointer),
                since: Some(now),
            });
            // The first frame of a drift is the one that starts the clock;
            // there is no interval yet to travel over.
            let Some(last) = drift.since else {
                window.request_animation_frame();
                return;
            };

            let step = (now - last).min(DRIFT_STEP).as_secs_f32();
            let (offset, max) = (handle.offset(), handle.max_offset());
            let moved = point(
                (offset.x + px(velocity.x * step)).clamp(-max.x, px(0.0)),
                (offset.y + px(velocity.y * step)).clamp(-max.y, px(0.0)),
            );
            if moved == offset {
                // Held at an end that has nowhere left to go. Asking for
                // another frame here would spin at the display's rate for as
                // long as the drag is held; the next pointer move draws one
                // anyway, and that is the soonest this could have anything to
                // do.
                return;
            }
            handle.set_offset(moved);
            window.request_animation_frame();
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
    .into_any_element()
}
