//! Drift: a pane that keeps moving while a drag is held at its edge.

use super::*;

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
pub(super) const DRIFT_STEP: Duration = Duration::from_millis(50);

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
pub(super) struct Drift {
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
