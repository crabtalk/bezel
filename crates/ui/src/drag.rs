//! Drag and drop reordering over gpui's active drag.
//!
//! The gesture is gpui's: a [`Handle`] puts `on_drag` on the host's element,
//! and the payload is a [`Carry`]. Any element can take part in the same drag
//! with `on_drag_move::<Carry<I>>` and `on_drop::<Carry<I>>` —
//! [`crate::docking`] does.
//!
//! A [`Domain`] adds landing, displacement and edge scrolling for the
//! [`Region`]s it builds. The host lays out, scrolls and virtualizes each
//! region's content; a region measures the handles painted inside it in the
//! frame being drawn, and nothing else. A landing names its neighbours by id,
//! so a list with rows that were not painted still gets an exact place.
//!
//! Keep one domain on the view that renders all of its regions.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    AnyElement, App, Axis, ElementId, IntoElement, Pixels, Point, Render,
    StatefulInteractiveElement, Window, prelude::*,
};
use motion::Painter;

mod element;
mod state;

pub use element::{Handle, Region};
pub(crate) use state::Gesture;
use state::State;

/// gpui's drag payload for an item of a [`Domain`].
pub struct Carry<I> {
    item: I,
    pub(crate) gesture: Rc<Gesture>,
}

impl<I> Carry<I> {
    pub fn item(&self) -> &I {
        &self.item
    }

    /// Whether a region of the source domain holds the pointer. A target
    /// outside the domain takes the item only while this is false.
    pub fn claimed(&self) -> bool {
        self.gesture.claimed.get()
    }

    /// A compact frame for a ghost at `pointer`, keeping the grab point.
    pub fn ghost_bounds(&self, pointer: Point<Pixels>) -> gpui::Bounds<Pixels> {
        self.gesture.ghost_bounds(pointer)
    }

    /// Hides the in-place item while unclaimed. For a target that draws its
    /// own ghost at `origin`; called on every frame the target shows it.
    pub fn host(&self, origin: Point<Pixels>) {
        self.gesture.host(origin);
    }
}

/// Where a released item goes. `after` and `before` are its neighbours among
/// the items painted in `region` at release; either is `None` at an end of
/// what was painted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drop<R, I> {
    pub item: I,
    pub from: R,
    pub region: R,
    pub after: Option<I>,
    pub before: Option<I>,
}

/// A release over no region that accepts the item. Nothing is committed.
#[derive(Clone, Debug)]
pub struct Outside<I> {
    pub item: I,
    pub position: Point<Pixels>,
}

/// How a region shows where the item would land.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Feedback {
    /// Neighbours slide aside to open a gap.
    #[default]
    Displace,
    /// A line in the gap; nothing moves.
    Indicator,
}

type Ghost<I> = dyn Fn(&I, &mut Window, &mut App) -> AnyElement;
type Paint = dyn Fn(&mut Window, &mut App) -> AnyElement;

/// Gesture and animation state for the regions items move between.
pub struct Domain<R, I>(Rc<RefCell<State<R, I>>>);

impl<R, I> Clone for Domain<R, I> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<R: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> Domain<R, I> {
    /// The carried item is painted in place, following the pointer.
    pub fn new(painter: Painter) -> Self {
        Self(Rc::new(RefCell::new(State::new(painter, None))))
    }

    /// The carried item's own element stays hidden in its slot and gpui paints
    /// `ghost` under the pointer instead. For virtualized regions, where the
    /// item's element can scroll out of the painted range.
    pub fn with_ghost(
        painter: Painter,
        ghost: impl Fn(&I, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self(Rc::new(RefCell::new(State::new(
            painter,
            Some(Rc::new(ghost)),
        ))))
    }

    /// A drop target laid out by the host. `child` holds this region's handles.
    pub fn region(
        &self,
        element_id: impl Into<ElementId>,
        id: R,
        axis: Axis,
        child: impl IntoElement,
    ) -> Region<R, I> {
        self.0.borrow_mut().begin_frame();
        Region::new(
            self.clone(),
            element_id.into(),
            id,
            axis,
            child.into_any_element(),
        )
    }

    /// Makes `element` pickable as `item`. Item ids are unique within a domain.
    /// Custom controls inside it stop mouse-down propagation to keep their
    /// presses from starting a drag.
    pub fn handle<E>(&self, item: I, element: E) -> Handle<R, I>
    where
        E: StatefulInteractiveElement + IntoElement + 'static,
    {
        let domain = self.clone();
        let carry = Carry {
            item: item.clone(),
            gesture: Rc::new(Gesture::default()),
        };
        let element = element.on_drag(carry, move |carry, grab, window, cx| {
            domain.start(carry, grab, window, cx)
        });
        Handle::new(self.clone(), item, element.into_any_element())
    }

    /// An item that is measured and moves aside like a handle but cannot be
    /// picked up, such as a heading between sections.
    pub fn fixed(&self, item: I, element: impl IntoElement) -> Handle<R, I> {
        Handle::new(self.clone(), item, element.into_any_element())
    }

    /// Whether `item` is being carried.
    pub fn carries(&self, item: &I) -> bool {
        self.0
            .borrow()
            .live
            .as_ref()
            .is_some_and(|live| &live.item == item)
    }

    /// Ends the gesture without a drop.
    pub fn cancel(&self, window: &mut Window, cx: &mut App) {
        let live = self.0.borrow_mut().end(window, cx);
        if live.is_some() {
            cx.stop_active_drag(window);
        }
    }

    /// The item being carried.
    pub fn carried(&self) -> Option<I> {
        self.0.borrow().live.as_ref().map(|live| live.item.clone())
    }

    /// Ends the gesture at the next render that has a window.
    pub(crate) fn invalidate(&self) {
        self.0.borrow_mut().stale = true;
    }

    fn start(
        &self,
        carry: &Carry<I>,
        cursor: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Entity<Ghosted> {
        let (grab, ghost) = {
            let mut state = self.0.borrow_mut();
            let grab = state.start(&carry.item, carry.gesture.clone(), cursor, window, cx);
            (grab, state.ghost.clone())
        };
        let item = carry.item.clone();
        let gesture = carry.gesture.clone();
        cx.new(|_| Ghosted {
            gesture,
            // gpui places the view at the pointer less `cursor`.
            shift: cursor - grab,
            render: ghost.map(|ghost| {
                Rc::new(move |window: &mut Window, cx: &mut App| ghost(&item, window, cx))
                    as Rc<Paint>
            }),
        })
    }
}

/// The view gpui paints under the pointer: the domain's ghost, or nothing.
pub struct Ghosted {
    gesture: Rc<Gesture>,
    shift: Point<Pixels>,
    render: Option<Rc<Paint>>,
}

impl Render for Ghosted {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let hosted = self.gesture.hosted.get() && !self.gesture.claimed.get();
        match self.render.as_ref().filter(|_| !hosted) {
            Some(render) => {
                let gesture = self.gesture.clone();
                // gpui lays the drag view out as a root, which drops its own
                // insets and margins, so the shift sits one level down.
                gpui::div()
                    .child(
                        gpui::div()
                            .relative()
                            .left(self.shift.x)
                            .top(self.shift.y)
                            .child(render(window, cx))
                            .child(
                                gpui::canvas(
                                    move |bounds, _, _| gesture.shown.set(Some(bounds.origin)),
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full(),
                            ),
                    )
                    .into_any_element()
            }
            None => gpui::Empty.into_any_element(),
        }
    }
}
