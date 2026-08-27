//! [`panel`] — content that floats over a page and is dragged around it: a
//! meter, an inspector, a detached preview.
//!
//! The panel lays a full-size layer over its container and places the box
//! inside it, because the drag has to be heard somewhere larger than the thing
//! being dragged: a pointer that outruns a frame lands outside a box-sized
//! hitbox, and the gesture stalls with the box stranded behind the cursor.
//! [`crate::scroll`] hangs its thumb drag off the track for the same reason.
//!
//! ```ignore
//! // The state is a field of the view that mounts it; the panel is one line.
//! div().relative().size_full()
//!     .child(page)
//!     .child(floating::panel("meter", &self.meter, home, self.stats.clone()))
//! ```
//!
//! It clamps nothing and remembers nothing across launches. A panel dragged
//! half off the window stays there, and the point it was grabbed by is under
//! the pointer, so it can always be dragged back.

use std::{cell::Cell, rc::Rc};

use std::time::Duration;

use gpui::{
    App, ElementId, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, SharedString, Styled as _,
    Window, div,
};
use motion::Painter;

/// Redraw rate while a panel is being dragged, beside [`motion`]'s 30 for
/// spinners and 60 for hover fades.
///
/// The rate is the point of the exercise: gpui's own drag redraws the whole
/// window once per *pointer sample*, and a pointer reports far faster than a
/// window can paint.
const DRAG_FPS: f32 = 60.0;

/// How long a claim outlives the move that took it. A pointer holding still
/// has nothing to paint, so the clock parks and the next move claims again.
const DRAG_LEASE: Duration = Duration::from_millis(200);

/// How far the pointer travels before a press becomes a drag — gpui's own
/// `DRAG_THRESHOLD`, which this gesture no longer goes through.
const THRESHOLD: f64 = 2.0;

/// Where a panel sits, and where inside it the pointer took hold. A field of
/// the view that mounts the panel, like [`crate::scroll::ScrollbarState`] —
/// and, for the same reason, carrying its own [`Painter`]: a drag runs in
/// event-dispatch context, where the window cannot resolve which view is
/// asking to be redrawn.
#[derive(Clone)]
pub struct Floating {
    /// `None` until the first drag: the panel opens at the `home` its host
    /// passes and only holds a position of its own once moved.
    at: Rc<Cell<Option<Point<Pixels>>>>,
    /// The last pointer sample this gesture accounted for. Movement is carried
    /// as a delta from it, and a delta reads the same from the window's corner
    /// or the container's — so the panel needs no element bounds to place
    /// itself, and none are on offer in a plain mouse listener.
    anchor: Rc<Cell<Point<Pixels>>>,
    /// The pointer is pressed on the panel, travelling or not. The hand closes
    /// here rather than on the first movement, which is where every other
    /// grabbable surface closes it.
    held: Rc<Cell<bool>>,
    /// The panel is being moved: past the threshold that separates a drag from
    /// a click.
    dragging: Rc<Cell<bool>>,
    painter: Painter,
}

impl Floating {
    pub fn new(painter: Painter) -> Self {
        Self {
            at: Rc::new(Cell::new(None)),
            anchor: Rc::new(Cell::new(Point::default())),
            held: Rc::new(Cell::new(false)),
            dragging: Rc::new(Cell::new(false)),
            painter,
        }
    }

    /// Where the panel sits, once it has been moved. Read it to persist a
    /// position; hand it back with [`Self::move_to`].
    pub fn at(&self) -> Option<Point<Pixels>> {
        self.at.get()
    }

    /// Whether the pointer is holding the panel, travelling or not — the
    /// closed hand.
    pub fn held(&self) -> bool {
        self.held.get()
    }

    /// Whether the panel is being moved, which a press alone is not — the lift,
    /// the shadow, whatever a host shows for a thing in flight.
    pub fn dragging(&self) -> bool {
        self.dragging.get()
    }

    pub fn move_to(&self, at: Point<Pixels>) {
        self.at.set(Some(at));
    }

    /// One pointer sample. Cheap on purpose: it moves the panel and claims a
    /// frame, and claiming is not redrawing — [`Painter::lease`] schedules,
    /// leaving the clock to paint at [`DRAG_FPS`] however fast the samples
    /// arrive.
    fn sample(&self, home: Point<Pixels>, pointer: Point<Pixels>, cx: &mut App) {
        if !self.held.get() {
            return;
        }
        let travelled = pointer - self.anchor.get();
        if !self.dragging.get() {
            if travelled.magnitude() <= THRESHOLD {
                return;
            }
            // The gesture starts here, so the first step is measured from here
            // and the panel does not jump by the threshold it just crossed.
            self.dragging.set(true);
            self.anchor.set(pointer);
            self.painter.notify(cx);
            return;
        }
        self.move_to(self.at.get().unwrap_or(home) + travelled);
        self.anchor.set(pointer);
        self.painter.lease(DRAG_FPS, DRAG_LEASE, cx);
    }
}

/// The panel: `child` floating where `state` left it, or at `home` until it is
/// dragged. Mount it in a `relative()` container — it lays a layer over that
/// container's whole box.
///
/// `home` is passed every render rather than stored, so a host can read it off
/// the viewport and a window that grows never strands the panel out of reach.
pub fn panel(
    id: impl Into<SharedString>,
    state: &Floating,
    home: Point<Pixels>,
    child: impl IntoElement,
) -> impl IntoElement {
    let id = id.into();
    let at = state.at.get().unwrap_or(home);

    let pressed = {
        let state = state.clone();
        move |event: &MouseDownEvent, _: &mut Window, cx: &mut App| {
            state.held.set(true);
            state.anchor.set(event.position);
            state.painter.notify(cx);
        }
    };
    let moved = {
        let state = state.clone();
        move |event: &MouseMoveEvent, _: &mut Window, cx: &mut App| {
            state.sample(home, event.position, cx);
        }
    };
    let released = {
        let state = state.clone();
        move |_: &MouseUpEvent, _: &mut Window, cx: &mut App| {
            // Both, separately: `||` would short-circuit past the second
            // cell and leave a panel dragging for the rest of its life.
            let held = state.held.replace(false);
            let dragging = state.dragging.replace(false);
            if held || dragging {
                state.painter.notify(cx);
            }
        }
    };

    let box_ = div()
        .absolute()
        .left(at.x)
        .top(at.y)
        .id(ElementId::from(id.clone()));
    let box_ = if state.held() || state.dragging() {
        box_.cursor_grabbing()
    } else {
        box_.cursor_grab()
    };

    div()
        .id(ElementId::from(SharedString::from(format!("{id}-layer"))))
        .absolute()
        .inset_0()
        .child(box_.on_mouse_down(MouseButton::Left, pressed).child(child))
        // The gesture is heard on the layer rather than the box: a pointer
        // moving faster than the frames that follow it is outside the box for
        // most of the drag, and a box-mounted listener would go quiet.
        .on_mouse_move(moved)
        .on_mouse_up(MouseButton::Left, released.clone())
        .on_mouse_up_out(MouseButton::Left, released)
}
