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

use gpui::{
    App, AppContext as _, DragMoveEvent, ElementId, Empty, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Point, SharedString, StatefulInteractiveElement as _, Styled as _,
    div,
};
use motion::Painter;

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
    grab: Rc<Cell<Point<Pixels>>>,
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
            grab: Rc::new(Cell::new(Point::default())),
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

    /// One drag move: the box's origin is the pointer less the point it was
    /// grabbed by, measured from the container it floats in.
    ///
    /// The layer's own origin is what carries that last part. A pointer is
    /// reported from the window's corner while `left`/`top` are laid out from
    /// the container's, so a panel mounted anywhere but the window's own origin
    /// leaves the page by exactly that distance on its first drag.
    fn drag(&self, id: &SharedString, event: &DragMoveEvent<Drag>, cx: &mut App) {
        // Another panel's box: `on_drag_move` filters by payload type, and
        // every panel in the window shares this one.
        if event.drag(cx).id != *id {
            return;
        }
        let grab = event.drag(cx).grab.get();
        self.move_to(event.event.position - event.bounds.origin - grab);
        self.painter.notify(cx);
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
    let (dragged, moving) = (state.clone(), id.clone());
    let released = {
        let state = state.clone();
        move |_: &gpui::MouseUpEvent, _: &mut gpui::Window, cx: &mut App| {
            // Both, separately: `||` would short-circuit past the second
            // cell and leave a panel dragging for the rest of its life.
            let held = state.held.replace(false);
            let dragging = state.dragging.replace(false);
            if held || dragging {
                state.painter.notify(cx);
            }
        }
    };
    let pressed = {
        let state = state.clone();
        move |_: &gpui::MouseDownEvent, _: &mut gpui::Window, cx: &mut App| {
            state.held.set(true);
            state.painter.notify(cx);
        }
    };
    let armed = state.clone();

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
        .absolute()
        .inset_0()
        .child(
            box_.on_mouse_down(gpui::MouseButton::Left, pressed)
                .on_drag(
                    Drag {
                        id,
                        grab: state.grab.clone(),
                    },
                    move |drag, offset, _, cx| {
                        drag.grab.set(offset);
                        armed.dragging.set(true);
                        armed.painter.notify(cx);
                        cx.new(|_| Empty)
                    },
                )
                .child(child),
        )
        .on_drag_move(move |event: &DragMoveEvent<Drag>, _, cx| {
            dragged.drag(&moving, event, cx);
        })
        // Both, because a release can land anywhere on screen; a hand left
        // closed reads as a panel still held.
        .on_mouse_up(gpui::MouseButton::Left, released.clone())
        .on_mouse_up_out(gpui::MouseButton::Left, released)
}

/// A panel drag: which panel, and where inside its box the pointer took hold.
/// The grab is filled as the drag starts, so the box tracks the pointer from
/// wherever it was taken hold of.
struct Drag {
    id: SharedString,
    grab: Rc<Cell<Point<Pixels>>>,
}
