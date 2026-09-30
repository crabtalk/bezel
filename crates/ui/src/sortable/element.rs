use std::rc::Rc;

use gpui::{
    AnyElement, App, Axis, Bounds, DispatchPhase, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, MouseButton, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, Window,
};
use motion::TAB_SLIDE;

use super::{OnMove, OnOutside, OutsideDrop, Sortable, state::Slide};

pub(super) struct Placed<L, I> {
    pub state: Sortable<L, I>,
    pub id: I,
    pub list: L,
    pub child: AnyElement,
    pub floating: bool,
}

impl<L: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> IntoElement for Placed<L, I> {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl<L: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> Element for Placed<L, I> {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let now = cx.background_executor().now();
        let mut state = self.state.0.borrow_mut();
        let list = state.list(&self.list).unwrap();
        let origin = list.origin;
        let axis = list.model.axis;
        let carried = if self.floating {
            state.drag.as_ref().map(|drag| {
                let mut origin = drag.pointer - drag.grab;
                if state.axis_locked {
                    match axis {
                        Axis::Horizontal => origin.y = drag.origin.y,
                        Axis::Vertical => origin.x = drag.origin.x,
                    }
                }
                origin
            })
        } else {
            None
        };
        let painter = state.painter;
        let slot = state.slot_mut(&self.id).unwrap();
        let offset = if let Some(origin) = carried {
            slot.slide = None;
            origin - bounds.origin
        } else {
            if let Some(previous) = slot.bounds {
                let scrolled = if slot.floating {
                    Point::default()
                } else {
                    origin - slot.origin
                };
                if previous.origin + scrolled != bounds.origin || slot.floating {
                    slot.slide = Some(Slide {
                        from: slot.painted + scrolled - bounds.origin,
                        since: now,
                    });
                }
            }
            if cx.reduce_motion() {
                slot.slide = None;
            }
            match slot.slide.as_ref().and_then(|slide| slide.offset(now)) {
                Some(offset) => offset,
                None => {
                    slot.slide = None;
                    Point::default()
                }
            }
        };
        slot.bounds = Some(bounds);
        slot.painted = bounds.origin + offset;
        slot.origin = origin;
        slot.floating = self.floating;
        let sliding = slot.slide.is_some();
        drop(state);
        if sliding {
            painter.lease(120., TAB_SLIDE.total(), cx);
        }
        window.with_element_offset(offset, |window| self.child.prepaint(window, cx));
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

pub(super) fn listen<L: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static>(
    state: Sortable<L, I>,
    moved: Option<Rc<OnMove<L, I>>>,
    outside: Option<Rc<OnOutside<I>>>,
    window: &mut Window,
) {
    let moving = state.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if phase != DispatchPhase::Capture {
            return;
        }
        let mut state = moving.0.borrow_mut();
        if event.pressed_button != Some(MouseButton::Left) {
            state.cancel(cx);
            state.restore_focus(window, cx);
            return;
        }
        let started = state.drag.as_ref().is_some_and(|drag| drag.started);
        if state.sample(event.position, cx) {
            if !started {
                state.previous_focus = window.focused(cx);
                state.focus_captured = true;
                if let Some(focus) = &state.focus {
                    window.focus(focus, cx);
                }
            }
            cx.stop_propagation();
        }
    });
    let released = state.clone();
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
            return;
        }
        let release = {
            let mut state = released.0.borrow_mut();
            if std::mem::take(&mut state.suppress_release) {
                cx.stop_propagation();
                return;
            }
            let release = state.release(event.position, cx);
            state.restore_focus(window, cx);
            release
        };
        if let Some((drag, movement)) = release {
            if let Some(movement) = movement
                && let Some(callback) = &moved
            {
                callback(&movement, window, cx);
            }
            if drag.to.is_none()
                && let Some(callback) = &outside
            {
                callback(
                    &OutsideDrop {
                        item: drag.item,
                        position: event.position,
                    },
                    window,
                    cx,
                );
            }
            cx.stop_propagation();
        }
    });
}

pub(super) struct MeasuredList<L, I> {
    pub state: Sortable<L, I>,
    pub list: L,
    pub viewport: bool,
    pub child: AnyElement,
}

impl<L: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> IntoElement
    for MeasuredList<L, I>
{
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl<L: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> Element
    for MeasuredList<L, I>
{
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // Measure the viewport before its scroll offset is applied to children.
        if let Some(list) = self.state.0.borrow_mut().list_mut(&self.list) {
            if self.viewport {
                list.viewport = bounds.intersect(&window.content_mask().bounds);
            } else {
                list.origin = bounds.origin;
            }
        }
        self.child.prepaint(window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}
