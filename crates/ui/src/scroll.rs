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

mod bar;
mod claim;
mod drift;
mod follow;
mod overlay;

pub use bar::*;
pub use claim::*;
pub use drift::*;
pub use follow::*;
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
