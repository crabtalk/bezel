//! Sortable lists with a shared drag, animated gaps, and one host mutation on drop.
//! Item ids must be unique within a group; list and item ids must remain stable.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    AnyElement, App, Axis, Bounds, ElementId, IntoElement, MouseButton, ParentElement, Pixels,
    Point, Refineable, RenderOnce, ScrollHandle, SharedString, StyleRefinement, Styled, Window,
    canvas, deferred, div, prelude::*, px,
};
use motion::Painter;

mod element;
mod state;
use element::Placed;
use state::{Model, State};

/// A list and an item or insertion index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Position<ListId> {
    pub list: ListId,
    pub index: usize,
}

/// Emitted once on release. Apply to host data synchronously.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move<ListId, ItemId> {
    pub item: ItemId,
    pub from: Position<ListId>,
    /// Insertion index after removing the source item.
    pub to: Position<ListId>,
}

/// A release outside every compatible list. No move is committed.
#[derive(Clone, Debug)]
pub struct OutsideDrop<ItemId> {
    pub item: ItemId,
    pub position: Point<Pixels>,
}

/// Internal handoff shared by tab strips and general sortable lists.
#[derive(Clone, PartialEq)]
pub(crate) struct Carry<I> {
    pub item: I,
    pub pointer: Point<Pixels>,
    pub bounds: Bounds<Pixels>,
    pub detached: bool,
}

pub(crate) type CancelCarry = dyn Fn(&mut Window, &mut App);

pub(crate) trait CarryTarget<I> {
    fn watch(&self, cancel: Rc<CancelCarry>);
    fn update(&self, carry: Carry<I>, cx: &mut App);
    fn cancel(&self, cx: &mut App);
    fn release(&self, carry: Carry<I>, window: &mut Window, cx: &mut App);
}

/// Shared state for all lists in one drag domain. Keep it on the owning view.
pub struct Sortable<ListId, ItemId>(Rc<RefCell<State<ListId, ItemId>>>);

impl<L, I> Clone for Sortable<L, I> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<L: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> Sortable<L, I> {
    pub fn new(painter: Painter) -> Self {
        Self(Rc::new(RefCell::new(State::new(painter))))
    }

    /// Mount every participating list together so removed lists cannot remain drop targets.
    pub fn group(
        &self,
        id: impl Into<ElementId>,
        lists: impl IntoIterator<Item = List<L, I>>,
    ) -> Group<L, I> {
        Group {
            id: id.into(),
            sortable: self.clone(),
            lists: lists.into_iter().collect(),
            moved: None,
            outside: None,
            style: StyleRefinement::default(),
            drag_style: StyleRefinement::default(),
            axis_locked: false,
            docking: None,
        }
    }

    /// Cancel a gesture without changing host data.
    pub fn cancel(&self, window: &mut Window, cx: &mut App) {
        let mut state = self.0.borrow_mut();
        state.cancel(cx);
        state.restore_focus(window, cx);
    }
}

/// One list's current data and appearance. Children are in host order.
pub struct List<L, I> {
    element_id: ElementId,
    id: L,
    axis: Axis,
    kind: SharedString,
    items: Vec<(I, AnyElement)>,
    gap: Pixels,
    scroll: Option<ScrollHandle>,
    header: Option<AnyElement>,
    viewport: Option<Box<Viewport>>,
    style: StyleRefinement,
}

impl<L, I> List<L, I> {
    pub fn new<E: IntoElement>(
        element_id: impl Into<ElementId>,
        id: L,
        axis: Axis,
        items: impl IntoIterator<Item = (I, E)>,
    ) -> Self {
        Self {
            element_id: element_id.into(),
            id,
            axis,
            kind: SharedString::default(),
            items: items
                .into_iter()
                .map(|(id, el)| (id, el.into_any_element()))
                .collect(),
            gap: px(0.),
            scroll: None,
            header: None,
            viewport: None,
            style: StyleRefinement::default(),
        }
    }

    /// Only lists with equal kinds exchange items. Untagged lists share one kind.
    pub fn kind(mut self, kind: impl Into<SharedString>) -> Self {
        self.kind = kind.into();
        self
    }

    pub(crate) fn wrap_viewport(
        mut self,
        wrap: impl FnOnce(AnyElement, &ScrollHandle) -> AnyElement + 'static,
    ) -> Self {
        self.viewport = Some(Box::new(wrap));
        self
    }

    pub fn gap(mut self, gap: Pixels) -> Self {
        self.gap = gap;
        self
    }

    /// Reuse the host's scroll handle. Dragging near its visible edges scrolls it.
    pub fn track_scroll(mut self, handle: &ScrollHandle) -> Self {
        self.scroll = Some(handle.clone());
        self
    }

    /// A fixed heading outside the scroll area.
    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }
}

impl<L, I> Styled for List<L, I> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

type Viewport = dyn FnOnce(AnyElement, &ScrollHandle) -> AnyElement;

type OnMove<L, I> = dyn Fn(&Move<L, I>, &mut Window, &mut App);
type OnOutside<I> = dyn Fn(&OutsideDrop<I>, &mut Window, &mut App);

/// A group lays its lists in a row by default; style it as any flex container.
#[derive(IntoElement)]
pub struct Group<L: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> {
    id: ElementId,
    sortable: Sortable<L, I>,
    lists: Vec<List<L, I>>,
    moved: Option<Rc<OnMove<L, I>>>,
    outside: Option<Rc<OnOutside<I>>>,
    style: StyleRefinement,
    axis_locked: bool,
    drag_style: StyleRefinement,
    docking: Option<Rc<dyn CarryTarget<I>>>,
}

impl<L: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> Group<L, I> {
    pub fn on_drop(mut self, moved: impl Fn(&Move<L, I>, &mut Window, &mut App) + 'static) -> Self {
        self.moved = Some(Rc::new(moved));
        self
    }

    pub fn on_drop_outside(
        mut self,
        outside: impl Fn(&OutsideDrop<I>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.outside = Some(Rc::new(outside));
        self
    }

    /// Hand detached items to a pane docking surface.
    pub fn docking<P: Clone + PartialEq + 'static>(
        mut self,
        dock: &crate::docking::Dock<P, I>,
    ) -> Self {
        self.docking = Some(Rc::new(dock.clone()));
        self
    }

    /// Additional appearance for the carried element, such as a surface or shadow.
    pub fn drag_style(mut self, style: StyleRefinement) -> Self {
        self.drag_style = style;
        self
    }

    /// For a single strip: keep its carried item on the list's axis.
    pub fn axis_locked(mut self) -> Self {
        self.axis_locked = true;
        self
    }
}

impl<L: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> Styled for Group<L, I> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<L: Clone + PartialEq + 'static, I: Clone + PartialEq + 'static> RenderOnce for Group<L, I> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window.use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle());
        let focus = focus.read(cx).clone();
        self.sortable.0.borrow_mut().focus = Some(focus.clone());
        let enabled = self.moved.is_some() || self.docking.is_some() || self.outside.is_some();
        {
            let mut state = self.sortable.0.borrow_mut();
            if state.docking.is_some() && self.docking.is_none() {
                state.cancel(cx);
            }
            state.docking = self.docking;
        }
        let models = self
            .lists
            .iter()
            .map(|list| Model {
                id: list.id.clone(),
                axis: list.axis,
                kind: list.kind.clone(),
                gap: list.gap,
                items: list.items.iter().map(|(id, _)| id.clone()).collect(),
            })
            .collect();
        self.sortable
            .0
            .borrow_mut()
            .sync(models, self.axis_locked, cx);
        if self.sortable.0.borrow().drag.is_none() {
            self.sortable.0.borrow_mut().restore_focus(window, cx);
        }
        if self
            .sortable
            .0
            .borrow()
            .drag
            .as_ref()
            .is_some_and(|drag| drag.started)
            && let Some(target) = &self.sortable.0.borrow().docking
        {
            let weak = Rc::downgrade(&self.sortable.0);
            target.watch(Rc::new(move |window, cx| {
                if let Some(state) = weak.upgrade() {
                    Sortable(state).cancel(window, cx);
                }
            }));
        }
        let cancelled = self.sortable.clone();
        let mut root = div()
            .id(self.id)
            .track_focus(&focus)
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key == "escape" && cancelled.0.borrow().drag.is_some() {
                    cancelled.cancel(window, cx);
                    cx.stop_propagation();
                }
            })
            .relative()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_row();
        root.style().refine(&self.style);
        let mut floating = None;
        for list in self.lists {
            let (drag, scroll) = {
                let mut state = self.sortable.0.borrow_mut();
                let meta = state.list_mut(&list.id).unwrap();
                if let Some(scroll) = list.scroll {
                    meta.scroll = scroll;
                }
                (
                    state.drag.clone().filter(|drag| drag.started),
                    state.list(&list.id).unwrap().scroll.clone(),
                )
            };
            let target = drag
                .as_ref()
                .and_then(|drag| drag.to.as_ref())
                .filter(|to| to.list == list.id);
            let mut children = Vec::new();
            for (id, child) in list.items {
                let serial = self.sortable.0.borrow_mut().serial(&id);
                let state = self.sortable.clone();
                let pressed = id.clone();
                let mut child = div()
                    .id(("sortable-item", serial))
                    .flex_none()
                    .child(child)
                    .when(enabled, |el| {
                        el.on_mouse_down(MouseButton::Left, move |event, _, cx| {
                            state.0.borrow_mut().press(&pressed, event.position, cx);
                        })
                    });
                let carried = drag.as_ref().is_some_and(|drag| drag.item == id);
                if carried {
                    child = child.cursor_grabbing();
                    child.style().refine(&self.drag_style);
                    if drag.as_ref().is_some_and(|drag| drag.detached)
                        && self.sortable.0.borrow().docking.is_some()
                    {
                        child = child.opacity(0.);
                    }
                }
                let child = Placed {
                    state: self.sortable.clone(),
                    id,
                    list: list.id.clone(),
                    child: child.into_any_element(),
                    floating: carried,
                };
                if carried {
                    let carried = div()
                        .absolute()
                        .cursor_grabbing()
                        .w(drag.as_ref().unwrap().size.width)
                        .h(drag.as_ref().unwrap().size.height)
                        .child(child);
                    floating = Some(deferred(carried));
                } else {
                    let returning = self
                        .sortable
                        .0
                        .borrow()
                        .slot(&child.id)
                        .is_some_and(|slot| slot.floating || slot.returning);
                    children.push(if returning {
                        deferred(child).into_any_element()
                    } else {
                        child.into_any_element()
                    });
                }
            }
            if let Some(target) = target {
                let size = drag.as_ref().unwrap().size;
                let gap = div()
                    .flex_none()
                    .when(list.axis == Axis::Horizontal, |el| {
                        el.w(size.width).h(size.height)
                    })
                    .when(list.axis == Axis::Vertical, |el| el.h(size.height).w_full());
                children.insert(target.index.min(children.len()), gap.into_any_element());
            }
            let content = div()
                .relative()
                .flex_none()
                .flex()
                .gap(list.gap)
                .when(list.axis == Axis::Horizontal, |el| el.flex_row())
                .when(list.axis == Axis::Vertical, |el| el.flex_col().w_full())
                .children(children);
            let content = element::MeasuredList {
                state: self.sortable.clone(),
                list: list.id.clone(),
                viewport: false,
                child: content.into_any_element(),
            };
            let viewport = div()
                .id("viewport")
                .relative()
                .min_w_0()
                .min_h_0()
                .flex_1()
                .track_scroll(&scroll)
                .flex()
                .when(list.axis == Axis::Horizontal, |el| {
                    el.flex_row().overflow_x_scroll()
                })
                .when(list.axis == Axis::Vertical, |el| {
                    el.flex_col().overflow_y_scroll()
                })
                .child(content);
            let viewport = element::MeasuredList {
                state: self.sortable.clone(),
                list: list.id,
                viewport: true,
                child: viewport.into_any_element(),
            };
            let viewport = match list.viewport {
                Some(wrap) => wrap(viewport.into_any_element(), &scroll),
                None => viewport.into_any_element(),
            };
            let mut frame = div()
                .id(list.element_id)
                .min_w_0()
                .min_h_0()
                .flex()
                .flex_col();
            frame.style().refine(&list.style);
            root = root.child(frame.children(list.header).child(viewport));
        }
        let state = self.sortable;
        let measure = state.clone();
        root.children(floating).child(
            canvas(
                move |_, window, cx| {
                    let mut state = measure.0.borrow_mut();
                    if state.aim() {
                        let painter = state.painter;
                        window.defer(cx, move |_, cx| painter.notify(cx));
                    }
                    state.drift(cx);
                    state.report_carry(cx);
                },
                move |_, _, window, _| element::listen(state, self.moved, self.outside, window),
            )
            .absolute()
            .inset_0(),
        )
    }
}
