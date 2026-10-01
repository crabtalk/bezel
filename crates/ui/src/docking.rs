//! Pane docking previews. Hosts own the layout and apply one accepted drop.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    AnyElement, App, Bounds, ElementId, IntoElement, Pixels, Point, Window, point, px, size,
};
use motion::{Painter, TAB_SLIDE};
use web_time::Instant;

mod element;
pub use element::{Pane, Surface};

/// The destination within a pane. The bar and centre both join its tab strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Join,
    Left,
    Right,
    Top,
    Bottom,
}

/// Hit-test a pane in window coordinates. Corners choose the nearest normalized edge.
pub fn zone(bounds: Bounds<Pixels>, bar_height: Pixels, pointer: Point<Pixels>) -> Option<Zone> {
    if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) || !bounds.contains(&pointer) {
        return None;
    }
    if pointer.y < bounds.top() + bar_height.max(px(0.)) {
        return Some(Zone::Join);
    }
    let x = (pointer.x - bounds.left()) / bounds.size.width;
    let y = (pointer.y - bounds.top()) / bounds.size.height;
    let (distance, edge) = [
        (x, Zone::Left),
        (1. - x, Zone::Right),
        (y, Zone::Top),
        (1. - y, Zone::Bottom),
    ]
    .into_iter()
    .min_by(|a, b| a.0.total_cmp(&b.0))
    .unwrap();
    Some(if distance < 0.25 { edge } else { Zone::Join })
}

/// The inset preview rectangle for a destination.
pub fn preview_bounds(mut bounds: Bounds<Pixels>, zone: Zone) -> Bounds<Pixels> {
    match zone {
        Zone::Join => {}
        Zone::Left => bounds.size.width /= 2.,
        Zone::Right => {
            bounds.size.width /= 2.;
            bounds.origin.x += bounds.size.width;
        }
        Zone::Top => bounds.size.height /= 2.,
        Zone::Bottom => {
            bounds.size.height /= 2.;
            bounds.origin.y += bounds.size.height;
        }
    }
    let inset = px(4.)
        .min(bounds.size.width / 2.)
        .min(bounds.size.height / 2.);
    bounds.origin += point(inset, inset);
    bounds.size.width -= 2. * inset;
    bounds.size.height -= 2. * inset;
    bounds
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drop<PaneId, ItemId> {
    pub item: ItemId,
    pub pane: PaneId,
    pub zone: Zone,
}

/// Keep one controller on the view that renders the whole docking surface.
pub struct Dock<P, I>(Rc<RefCell<State<P, I>>>);

impl<P, I> Clone for Dock<P, I> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

type Ghost<I> = dyn Fn(&I, &mut Window, &mut App) -> AnyElement;
type OnDrop<P, I> = dyn Fn(&Drop<P, I>, &mut Window, &mut App) -> Option<P>;

struct Target<P> {
    id: P,
    bounds: Bounds<Pixels>,
    visible: Bounds<Pixels>,
    bar_height: Pixels,
}

struct Tween {
    from: Bounds<Pixels>,
    to: Bounds<Pixels>,
    since: Instant,
}

impl Tween {
    fn progress(&self, cx: &App) -> f32 {
        if cx.reduce_motion() {
            return 1.;
        }
        let elapsed = cx
            .background_executor()
            .now()
            .saturating_duration_since(self.since);
        TAB_SLIDE.progress(
            elapsed.as_secs_f32()
                / TAB_SLIDE
                    .total()
                    .mul_f32(motion::speed_scale())
                    .as_secs_f32(),
        )
    }
    fn bounds(&self, cx: &App) -> Bounds<Pixels> {
        let t = self.progress(cx);
        Bounds::new(
            self.from.origin + (self.to.origin - self.from.origin) * t,
            size(
                self.from.size.width + (self.to.size.width - self.from.size.width) * t,
                self.from.size.height + (self.to.size.height - self.from.size.height) * t,
            ),
        )
    }
}

struct Settling<P, I> {
    pane: P,
    item: I,
    preview: Bounds<Pixels>,
    ghost: Bounds<Pixels>,
    since: Instant,
}

struct Carried<I> {
    item: I,
    gesture: Rc<crate::drag::Gesture>,
    pointer: Point<Pixels>,
}

impl<I> Carried<I> {
    /// Off every region of its domain: this surface's to take.
    fn detached(&self) -> bool {
        !self.gesture.claimed.get()
    }

    fn ghost_bounds(&self) -> Bounds<Pixels> {
        self.gesture.ghost_bounds(self.pointer)
    }
}

struct State<P, I> {
    painter: Painter,
    ghost: Rc<Ghost<I>>,
    dropped: Option<Rc<OnDrop<P, I>>>,
    targets: Vec<Target<P>>,
    carried: Option<Carried<I>>,
    landing: Option<(P, Zone)>,
    preview: Option<Tween>,
    settling: Option<Settling<P, I>>,
}

impl<P: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> Dock<P, I> {
    /// `ghost` renders the detached item and its brief settle after a drop.
    pub fn new(
        painter: Painter,
        ghost: impl Fn(&I, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self(Rc::new(RefCell::new(State {
            painter,
            ghost: Rc::new(ghost),
            dropped: None,
            targets: Vec::new(),
            carried: None,
            landing: None,
            preview: None,
            settling: None,
        })))
    }

    /// Mount every target inside this surface, in the controller's owning view.
    pub fn surface(&self, id: impl Into<ElementId>, child: impl IntoElement) -> Surface<P, I> {
        element::Surface {
            dock: self.clone(),
            id: id.into(),
            child: child.into_any_element(),
            dropped: None,
        }
    }

    /// Measure a pane and animate its entry after an accepted drop.
    pub fn pane(&self, id: P, bar_height: Pixels, child: impl IntoElement) -> Pane<P, I> {
        element::Pane {
            dock: self.clone(),
            id,
            bar_height,
            child: child.into_any_element(),
        }
    }
}

impl<P: Clone + PartialEq, I: Clone + PartialEq> State<P, I> {
    fn aim(&mut self, cx: &mut App) {
        let target = self
            .carried
            .as_ref()
            .filter(|carried| carried.detached())
            .and_then(|carried| {
                self.targets.iter().rev().find_map(|target| {
                    if !target.visible.contains(&carried.pointer) {
                        return None;
                    }
                    zone(target.bounds, target.bar_height, carried.pointer)
                        .map(|zone| (target.id.clone(), zone, preview_bounds(target.bounds, zone)))
                })
            });
        let Some((pane, zone, bounds)) = target else {
            self.landing = None;
            self.preview = None;
            return;
        };
        if self
            .preview
            .as_ref()
            .is_none_or(|preview| preview.to != bounds)
        {
            let from = self
                .preview
                .as_ref()
                .map(|preview| preview.bounds(cx))
                .unwrap_or(bounds);
            self.preview = Some(Tween {
                from,
                to: bounds,
                since: cx.background_executor().now(),
            });
        }
        self.landing = Some((pane, zone));
    }
}

impl<P: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> Dock<P, I> {
    fn update(
        &self,
        item: I,
        gesture: Rc<crate::drag::Gesture>,
        pointer: Point<Pixels>,
        cx: &mut App,
    ) {
        let mut state = self.0.borrow_mut();
        state.carried = Some(Carried {
            item,
            gesture,
            pointer,
        });
        state.settling = None;
        state.aim(cx);
        state.painter.notify(cx);
    }

    fn clear(&self, cx: &mut App) {
        let mut state = self.0.borrow_mut();
        if state.carried.take().is_some() {
            state.landing = None;
            state.preview = None;
            state.painter.notify(cx);
        }
    }

    fn release(
        &self,
        carry: &crate::drag::Carry<I>,
        pointer: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.update(carry.item().clone(), carry.gesture.clone(), pointer, cx);
        let proposal = {
            let state = self.0.borrow();
            let carried = state.carried.as_ref().filter(|carried| carried.detached());
            carried
                .zip(state.landing.as_ref())
                .zip(state.dropped.as_ref())
                .map(|((carried, (pane, zone)), callback)| {
                    (
                        Drop {
                            item: carried.item.clone(),
                            pane: pane.clone(),
                            zone: *zone,
                        },
                        callback.clone(),
                        state.preview.as_ref().unwrap().bounds(cx),
                        carried.ghost_bounds(),
                    )
                })
        };
        self.clear(cx);
        if let Some((event, callback, preview, ghost)) = proposal
            && let Some(pane) = callback(&event, window, cx)
        {
            let mut state = self.0.borrow_mut();
            state.settling = Some(Settling {
                pane,
                item: event.item,
                preview,
                ghost,
                since: cx.background_executor().now(),
            });
            state.painter.notify(cx);
        }
    }
}
