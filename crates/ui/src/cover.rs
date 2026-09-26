//! Where overlays sit in the frame being drawn, for native views over the
//! window (a webview) that nothing gpui paints can cover.
//!
//! A cover is a rectangle recorded during prepaint, stamped with its place in
//! the frame's prepaint order. gpui prepaints an element that paints over
//! another after that other one: a later sibling, a `deferred` layer, the
//! tooltip, the drag preview. So a native view takes a [`mark`] in its own
//! prepaint and asks [`covered`] in its paint, when every cover of the frame
//! is recorded.
//!
//! [`crate::surface`]'s cards and the dialog and sheet scrims record
//! themselves. Anything else an app floats puts a [`cover`] in it.
//!
//! A cover in a view gpui reuses from the last frame (`AnyView::cached`) is not
//! recorded.

use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};

use gpui::{App, Bounds, Global, IntoElement, Pixels, Styled, Window, WindowId, canvas};

/// A place in a frame's prepaint order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Mark(u64);

/// An element filling its parent, recorded as a cover. Its parent's box is
/// what it records, so it goes in the overlay's outermost box.
pub fn cover() -> impl IntoElement {
    canvas(record, |_, _, _, _| {}).absolute().size_full()
}

/// Records `bounds` as covered. Call it from prepaint.
pub fn record(bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let bounds = bounds.intersect(&window.content_mask().bounds);
    let mark = next();
    let id = window.window_handle().window_id();
    let covers = cx.default_global::<Covers>();
    if let Some(frame) = covers.0.get_mut(&id) {
        frame.push((mark, bounds));
        return;
    }
    covers.0.insert(id, vec![(mark, bounds)]);
    // Runs before the next frame is drawn.
    window.on_next_frame(move |_, cx| {
        cx.default_global::<Covers>().0.remove(&id);
    });
}

/// The current place in the prepaint order. Call it from prepaint.
pub fn mark() -> Mark {
    next()
}

/// Whether a cover recorded after `mark` in this frame overlaps `bounds`. Call
/// it from paint.
pub fn covered(mark: Mark, bounds: Bounds<Pixels>, window: &Window, cx: &App) -> bool {
    cx.try_global::<Covers>()
        .and_then(|covers| covers.0.get(&window.window_handle().window_id()))
        .is_some_and(|frame| {
            frame
                .iter()
                .any(|(at, cover)| *at > mark && cover.intersects(&bounds))
        })
}

fn next() -> Mark {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    Mark(NEXT.fetch_add(1, Ordering::Relaxed))
}

/// This frame's covers, per window.
#[derive(Default)]
struct Covers(HashMap<WindowId, Vec<(Mark, Bounds<Pixels>)>>);

impl Global for Covers {}
