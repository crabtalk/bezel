use super::{Dock, OnDrop, Target, Tween};
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, ContentMask, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, RenderOnce, Window, div, prelude::*, px,
};
use std::rc::Rc;

#[derive(IntoElement)]
pub struct Surface<P: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> {
    pub(super) dock: Dock<P, I>,
    pub(super) id: ElementId,
    pub(super) child: AnyElement,
    pub(super) dropped: Option<Rc<OnDrop<P, I>>>,
}

impl<P: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> Surface<P, I> {
    /// Apply the split/join synchronously and return its resulting pane; `None` rejects it.
    pub fn on_drop(
        mut self,
        callback: impl Fn(&super::Drop<P, I>, &mut Window, &mut App) -> Option<P> + 'static,
    ) -> Self {
        self.dropped = Some(Rc::new(callback));
        self
    }
}

impl<P: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> RenderOnce for Surface<P, I> {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.dock.0.borrow_mut().dropped = self.dropped;
        SurfaceElement {
            dock: self.dock,
            child: div()
                .id(self.id)
                .size_full()
                .child(self.child)
                .into_any_element(),
        }
    }
}

struct SurfaceElement<P, I> {
    dock: Dock<P, I>,
    child: AnyElement,
}

impl<P: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> IntoElement
    for SurfaceElement<P, I>
{
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl<P: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> Element
    for SurfaceElement<P, I>
{
    type RequestLayoutState = ();
    type PrepaintState = Option<AnyElement>;
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
    ) -> Option<AnyElement> {
        {
            let mut state = self.dock.0.borrow_mut();
            state.targets.clear();
            state.carry_seen = false;
        }
        self.child.prepaint(window, cx);
        // A source removed from this surface cannot leave a live preview behind.
        let cancel = {
            let mut state = self.dock.0.borrow_mut();
            if state.carry.is_some() && !state.carry_seen {
                state.carry = None;
                state.preview = None;
                state.landing = None;
                state.cancel_source.take()
            } else {
                None
            }
        };
        if let Some(cancel) = cancel {
            cancel(window, cx);
        }
        let mut state = self.dock.0.borrow_mut();
        state.aim(cx);
        let theme = theme::Theme::of(cx);
        let mut overlay = div()
            .absolute()
            .w(bounds.size.width)
            .h(bounds.size.height)
            .overflow_hidden();
        let mut active = false;
        if let Some(preview) = &state.preview {
            let rect = preview.bounds(cx);
            let offset = rect.origin - bounds.origin;
            overlay = overlay.child(
                div()
                    .debug_selector(|| "dock-preview".into())
                    .absolute()
                    .left(offset.x)
                    .top(offset.y)
                    .w(rect.size.width)
                    .h(rect.size.height)
                    .rounded(px(6.))
                    .bg(theme.accent.opacity(0.16))
                    .border_2()
                    .border_color(theme.accent.opacity(0.7)),
            );
            active |= preview.progress(cx) < 1.;
        }
        let ghost = state.settling.as_ref().and_then(|settle| {
            state
                .targets
                .iter()
                .find(|target| target.id == settle.pane)
                .map(|target| {
                    let tween = Tween {
                        from: settle.ghost,
                        to: Bounds::new(target.bounds.origin, settle.ghost.size),
                        since: settle.since,
                    };
                    (settle.item.clone(), tween.bounds(cx), tween.progress(cx))
                })
        });
        let carried = state
            .carry
            .as_ref()
            .filter(|carry| carry.detached)
            .map(|carry| (carry.item.clone(), carry.bounds, 0.));
        let render_ghost = state.ghost.clone();
        if let Some((_, _, progress)) = &ghost {
            if *progress >= 1. {
                state.settling = None;
            } else {
                active = true;
            }
        } else {
            state.settling = None;
        }
        if active {
            state.painter.lease(120., motion::TAB_SLIDE.total(), cx);
        }
        drop(state);
        if let Some((item, rect, progress)) = carried.or(ghost)
            && progress < 1.
        {
            let offset = rect.origin - bounds.origin;
            let selector = if progress == 0. && self.dock.0.borrow().carry.is_some() {
                "dock-carry-ghost"
            } else {
                "dock-settle-ghost"
            };
            overlay = overlay.child(
                div()
                    .debug_selector(move || selector.into())
                    .absolute()
                    .left(offset.x)
                    .top(offset.y)
                    .w(rect.size.width)
                    .h(rect.size.height)
                    .overflow_hidden()
                    .opacity(1. - progress)
                    .child(render_ghost(&item, window, cx)),
            );
        }
        let mut overlay = overlay.into_any_element();
        overlay.layout_as_root(bounds.size.map(AvailableSpace::Definite), window, cx);
        overlay.prepaint_at(bounds.origin, window, cx);
        Some(overlay)
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        overlay: &mut Option<AnyElement>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
        if let Some(overlay) = overlay {
            overlay.paint(window, cx);
        }
    }
}

/// A measured target; style its child to set its layout size.
pub struct Pane<P, I> {
    pub(super) dock: Dock<P, I>,
    pub(super) id: P,
    pub(super) bar_height: Pixels,
    pub(super) child: AnyElement,
}

impl<P: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> IntoElement for Pane<P, I> {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl<P: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> Element for Pane<P, I> {
    type RequestLayoutState = ();
    type PrepaintState = Bounds<Pixels>;
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
    ) -> Bounds<Pixels> {
        let mut state = self.dock.0.borrow_mut();
        state.targets.push(Target {
            id: self.id.clone(),
            bounds,
            visible: bounds.intersect(&window.content_mask().bounds),
            bar_height: self.bar_height,
        });
        let rect = state
            .settling
            .as_ref()
            .filter(|settle| settle.pane == self.id)
            .map(|settle| {
                Tween {
                    from: settle.preview,
                    to: bounds,
                    since: settle.since,
                }
                .bounds(cx)
            })
            .unwrap_or(bounds);
        drop(state);
        window.with_content_mask(
            Some(ContentMask {
                bounds: rect,
                ..Default::default()
            }),
            |window| {
                window.with_element_offset(rect.origin - bounds.origin, |window| {
                    self.child.prepaint(window, cx)
                });
            },
        );
        rect
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        rect: &mut Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_content_mask(
            Some(ContentMask {
                bounds: *rect,
                ..Default::default()
            }),
            |window| self.child.paint(window, cx),
        );
    }
}
