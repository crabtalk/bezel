//! Nesting: which pane a wheel belongs to.

use super::*;

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
pub(super) fn travel(handle: &ScrollHandle, axes: Axes) -> Point<Pixels> {
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
