use std::rc::Rc;

use gpui::{
    AnyElement, App, Axis, Bounds, DispatchPhase, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, Refineable, RenderOnce, ScrollHandle, StyleRefinement, Styled,
    Window, div, fill, point, prelude::*, px, size,
};

use super::{
    Domain, Drop, Feedback, Outside,
    state::{Accepts, Carries, Commit, Config, Lands, OnDrop, OnOutside},
};

/// A drop target whose content the host lays out. Style it as the box that
/// holds the content: its visible bounds are where it takes the pointer.
#[derive(IntoElement)]
pub struct Region<R: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> {
    domain: Domain<R, I>,
    element_id: ElementId,
    id: R,
    axis: Axis,
    child: AnyElement,
    accepts: Option<Box<Accepts<I>>>,
    lands: Option<Box<Lands<I>>>,
    carries: Option<Box<Carries<I>>>,
    feedback: Feedback,
    axis_locked: bool,
    scroll: Option<ScrollHandle>,
    dropped: Option<Rc<OnDrop<R, I>>>,
    outside: Option<Rc<OnOutside<I>>>,
    style: StyleRefinement,
}

impl<R: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> Region<R, I> {
    pub(super) fn new(
        domain: Domain<R, I>,
        element_id: ElementId,
        id: R,
        axis: Axis,
        child: AnyElement,
    ) -> Self {
        Self {
            domain,
            element_id,
            id,
            axis,
            child,
            accepts: None,
            lands: None,
            carries: None,
            feedback: Feedback::default(),
            axis_locked: false,
            scroll: None,
            dropped: None,
            outside: None,
            style: StyleRefinement::default(),
        }
    }

    /// Items this region takes. It takes every item of its domain by default.
    pub fn accepts(mut self, accepts: impl Fn(&I) -> bool + 'static) -> Self {
        self.accepts = Some(Box::new(accepts));
        self
    }

    /// Where in this region an item may land, by its neighbours there. The
    /// pointer's spot moves to the nearest allowed one among painted items;
    /// with none allowed the region holds the item without a landing, and a
    /// release commits nothing.
    pub fn lands(mut self, lands: impl Fn(&I, Option<&I>, Option<&I>) -> bool + 'static) -> Self {
        self.lands = Some(Box::new(lands));
        self
    }

    /// The items that go along with `item` when it is picked up here, in
    /// order after it. They are hidden with it and land with it.
    pub fn carries(mut self, carries: impl Fn(&I) -> Vec<I> + 'static) -> Self {
        self.carries = Some(Box::new(carries));
        self
    }

    pub fn feedback(mut self, feedback: Feedback) -> Self {
        self.feedback = feedback;
        self
    }

    /// An item carried out of this region stays on its axis until the pointer
    /// leaves it by 12px across that axis, and returns once it re-enters.
    pub fn axis_locked(mut self) -> Self {
        self.axis_locked = true;
        self
    }

    /// The handle scrolling this region's content. Holding an item near the
    /// region's visible edges scrolls it.
    pub fn track_scroll(mut self, handle: &ScrollHandle) -> Self {
        self.scroll = Some(handle.clone());
        self
    }

    /// Runs once on a release that lands in this region somewhere other than
    /// where the item started. Apply it synchronously.
    pub fn on_drop(
        mut self,
        dropped: impl Fn(&Drop<R, I>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.dropped = Some(Rc::new(dropped));
        self
    }

    /// Runs when an item carried out of this region is released over no
    /// region that accepts it.
    pub fn on_drop_outside(
        mut self,
        outside: impl Fn(&Outside<I>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.outside = Some(Rc::new(outside));
        self
    }
}

impl<R: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> Styled for Region<R, I> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<R: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> RenderOnce for Region<R, I> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window
            .use_keyed_state(self.element_id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let ended = {
            let state = self.domain.0.borrow();
            state.live.is_some() && (state.stale || !cx.has_active_drag())
        };
        if ended {
            self.domain.cancel(window, cx);
        }
        let config = Rc::new(Config {
            accepts: self.accepts,
            lands: self.lands,
            carries: self.carries,
            feedback: self.feedback,
            axis_locked: self.axis_locked,
            scroll: self.scroll,
            dropped: self.dropped,
            outside: self.outside,
            focus: focus.clone(),
        });
        let escaped = self.domain.clone();
        let mut frame =
            div()
                .id(self.element_id)
                .track_focus(&focus)
                .on_key_down(move |event, window, cx| {
                    if event.keystroke.key == "escape" && escaped.0.borrow().live.is_some() {
                        escaped.cancel(window, cx);
                        cx.stop_propagation();
                    }
                });
        frame.style().refine(&self.style);
        RegionElement {
            domain: self.domain,
            id: self.id,
            axis: self.axis,
            config,
            child: frame.child(self.child).into_any_element(),
        }
    }
}

struct RegionElement<R, I> {
    domain: Domain<R, I>,
    id: R,
    axis: Axis,
    config: Rc<Config<R, I>>,
    child: AnyElement,
}

impl<R: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> IntoElement
    for RegionElement<R, I>
{
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl<R: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> Element
    for RegionElement<R, I>
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
        let visible = bounds.intersect(&window.content_mask().bounds);
        let previous = self.domain.0.borrow_mut().enter(
            self.id.clone(),
            self.axis,
            bounds,
            visible,
            self.config.clone(),
        );
        self.child.prepaint(window, cx);
        let mut state = self.domain.0.borrow_mut();
        state.leave(previous);
        state.drift(cx);
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
        if self.config.feedback == Feedback::Indicator
            && let Some((axis, at, visible)) = self.domain.0.borrow().indicator(&self.id)
        {
            let line = match axis {
                Axis::Horizontal => Bounds::new(
                    point(at - px(1.), visible.origin.y),
                    size(px(2.), visible.size.height),
                ),
                Axis::Vertical => Bounds::new(
                    point(visible.origin.x, at - px(1.)),
                    size(visible.size.width, px(2.)),
                ),
            };
            window.paint_quad(fill(line, theme::Theme::of(cx).accent));
        }
        let moving = self.domain.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
            if phase != DispatchPhase::Capture {
                return;
            }
            let mut state = moving.0.borrow_mut();
            if state.aim(event.position) {
                state.painter.notify(cx);
            }
        });
        let released = self.domain.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                return;
            }
            let commit = released.0.borrow_mut().release(event.position, window, cx);
            match commit {
                Some(Commit::Drop(dropped, event)) => dropped(&event, window, cx),
                Some(Commit::Outside(outside, event)) => outside(&event, window, cx),
                None => {}
            }
        });
    }
}

/// An element that can be picked up, measured by the region it is painted in.
pub struct Handle<R, I> {
    domain: Domain<R, I>,
    item: I,
    child: Option<AnyElement>,
}

impl<R, I> Handle<R, I> {
    pub(super) fn new(domain: Domain<R, I>, item: I, child: AnyElement) -> Self {
        Self {
            domain,
            item,
            child: Some(child),
        }
    }
}

impl<R: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> IntoElement for Handle<R, I> {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

#[doc(hidden)]
pub struct Painted {
    origin: Point<Pixels>,
    hidden: bool,
}

impl<R: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> Element for Handle<R, I> {
    type RequestLayoutState = ();
    type PrepaintState = Painted;
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
        (self.child.as_mut().unwrap().request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Painted {
        let pointer = window.mouse_position();
        let placement = self
            .domain
            .0
            .borrow_mut()
            .place(&self.item, bounds, pointer, cx);
        window.with_element_offset(placement.offset, |window| {
            if placement.floating {
                let child = self.child.take().unwrap();
                let offset = window.element_offset();
                window.defer_draw(child, offset, 1, None);
            } else {
                self.child.as_mut().unwrap().prepaint(window, cx);
            }
        });
        Painted {
            origin: bounds.origin + placement.offset,
            hidden: placement.hidden,
        }
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        painted: &mut Painted,
        window: &mut Window,
        cx: &mut App,
    ) {
        let origin = painted.origin;
        let pressable = Bounds::new(origin, bounds.size);
        // Registered before the child's own, so it runs after them as the
        // press bubbles: the press is the handle's, not an ancestor's.
        window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
            if phase == DispatchPhase::Bubble
                && event.button == MouseButton::Left
                && pressable.contains(&event.position)
            {
                cx.stop_propagation();
            }
        });
        if let Some(child) = self.child.as_mut()
            && !painted.hidden
        {
            child.paint(window, cx);
        }
        let domain = self.domain.clone();
        let item = self.item.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, _, _| {
            if phase == DispatchPhase::Capture
                && event.button == MouseButton::Left
                && pressable.contains(&event.position)
            {
                domain.0.borrow_mut().press = Some((item.clone(), event.position - origin));
            }
        });
    }
}
