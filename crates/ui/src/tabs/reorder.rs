//! The horizontal, single-region adapter for [`crate::drag`].

use std::rc::Rc;

use gpui::{
    App, Axis, Div, ElementId, IntoElement, Pixels, Point, RenderOnce, ScrollHandle, Stateful,
    Window, div, prelude::*, px,
};
use motion::Painter;

use super::{GAP, Strip};
use crate::drag::{self, Domain};

/// Persistent gesture and animation state, one per strip.
pub struct Reorder<Id> {
    domain: Domain<(), Id>,
    scroll: ScrollHandle,
}

impl<Id> Clone for Reorder<Id> {
    fn clone(&self) -> Self {
        Self {
            domain: self.domain.clone(),
            scroll: self.scroll.clone(),
        }
    }
}

impl<Id: Clone + PartialEq + 'static> Reorder<Id> {
    pub fn new(painter: Painter) -> Self {
        Self {
            domain: Domain::new(painter),
            scroll: ScrollHandle::new(),
        }
    }

    /// Supply tabs in model order. The preview moves live; host order changes on release.
    pub fn bar(
        &self,
        id: impl Into<ElementId>,
        strip: &Strip<Id>,
        tabs: impl IntoIterator<Item = (Id, Stateful<Div>)>,
    ) -> Bar<Id> {
        let tabs: Vec<_> = tabs.into_iter().collect();
        debug_assert!(tabs.iter().map(|(id, _)| id).eq(strip.tabs()));
        if self
            .domain
            .carried()
            .is_some_and(|carried| !strip.contains(&carried))
        {
            self.domain.invalidate();
        }
        Bar {
            reorder: self.clone(),
            id: id.into(),
            tabs,
            moved: None,
            outside: None,
        }
    }
}

/// A committed move. The active tab remains the host's choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: usize,
    pub to: usize,
}

/// A carried tab released outside its strip; local reordering is cancelled.
#[derive(Clone, Debug)]
pub struct OutsideDrop<Id> {
    pub id: Id,
    pub position: Point<Pixels>,
}

type OnMove = dyn Fn(&Move, &mut Window, &mut App);
type OnOutside<Id> = dyn Fn(&OutsideDrop<Id>, &mut Window, &mut App);

#[derive(IntoElement)]
pub struct Bar<Id: Clone + PartialEq + 'static> {
    reorder: Reorder<Id>,
    id: ElementId,
    tabs: Vec<(Id, Stateful<Div>)>,
    moved: Option<Rc<OnMove>>,
    outside: Option<Rc<OnOutside<Id>>>,
}

impl<Id: Clone + PartialEq + 'static> Bar<Id> {
    /// Apply the final move synchronously. Accepts `cx.listener`.
    pub fn on_reorder(mut self, moved: impl Fn(&Move, &mut Window, &mut App) + 'static) -> Self {
        self.moved = Some(Rc::new(moved));
        self
    }

    pub fn on_drop_outside(
        mut self,
        outside: impl Fn(&OutsideDrop<Id>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.outside = Some(Rc::new(outside));
        self
    }
}

impl<Id: Clone + PartialEq + 'static> RenderOnce for Bar<Id> {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let raised = theme::Theme::of(cx).surface_raised;
        let Reorder { domain, scroll } = self.reorder;
        let order: Vec<Id> = self.tabs.iter().map(|(id, _)| id.clone()).collect();
        let row = div()
            .id("tab-row")
            .flex_1()
            .min_w_0()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(GAP))
            .overflow_x_scroll()
            .track_scroll(&scroll)
            .children(self.tabs.into_iter().map(|(id, tab)| {
                let tab = tab.flex_none();
                let tab = match domain.carries(&id) {
                    true => tab.bg(raised).cursor_grabbing(),
                    false => tab,
                };
                domain.handle(id, tab)
            }));
        let mut region = domain
            .region("tab-region", (), Axis::Horizontal, row)
            .axis_locked()
            .track_scroll(&scroll)
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex();
        if let Some(moved) = self.moved {
            region = region.on_drop(move |event: &drag::Drop<(), Id>, window, cx| {
                let Some(from) = order.iter().position(|id| *id == event.item) else {
                    return;
                };
                let rest: Vec<&Id> = order.iter().filter(|id| **id != event.item).collect();
                let to = match (&event.after, &event.before) {
                    (Some(after), _) => rest.iter().position(|id| *id == after).map(|at| at + 1),
                    (None, Some(before)) => rest.iter().position(|id| *id == before),
                    (None, None) => Some(0),
                };
                if let Some(to) = to {
                    moved(&Move { from, to }, window, cx);
                }
            });
        }
        if let Some(outside) = self.outside {
            region = region.on_drop_outside(move |event: &drag::Outside<Id>, window, cx| {
                outside(
                    &OutsideDrop {
                        id: event.item.clone(),
                        position: event.position,
                    },
                    window,
                    cx,
                );
            });
        }
        // Sized to its tabs: the bar grows into this box, not the row it sits in.
        div()
            .id(self.id)
            .relative()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_row()
            .child(super::bar("tab-bar").viewport(region.into_any_element(), &scroll))
    }
}
