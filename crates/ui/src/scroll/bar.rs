//! The bar: track, thumb, drag state and rail.

use super::*;

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

pub(super) type SetHover = Rc<dyn Fn(Hover, &mut Window, &mut App)>;

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

pub(super) fn track(id: &SharedString, place: Place, axis: Axis) -> Stateful<Div> {
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
            .w(px(TRACK)),
        Axis::Horizontal => el
            .left(BAR_INSET)
            .right(BAR_INSET + place.end)
            .bottom(place.near())
            .h(px(TRACK)),
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
pub(super) struct Place {
    /// Shortens the track at its far end.
    pub(super) end: Pixels,
    /// Gap between the pane's edge and the thumb, across the axis.
    pub(super) margin: Pixels,
}

impl Default for Place {
    fn default() -> Self {
        Self {
            end: px(0.),
            margin: px(MARGIN),
        }
    }
}

impl Place {
    /// Gap between the pane's edge and the track. The track is centred on the
    /// thumb where the margin leaves room, and starts at the edge otherwise.
    pub(super) fn near(self) -> Pixels {
        (self.margin - px(TRACK - THUMB) * 0.5).max(px(0.))
    }

    /// Gap between the track's outer side and the thumb.
    pub(super) fn inner(self) -> Pixels {
        self.margin - self.near()
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

pub(super) fn scrollbar_placed(
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
                .right(place.inner())
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

pub(super) fn transient_placed(
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
        .right(place.inner())
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
