//! The horizontal, single-list adapter for [`crate::sortable`].

use gpui::{
    App, Axis, Div, ElementId, IntoElement, Pixels, Point, RenderOnce, Stateful, StyleRefinement,
    Styled, Window, px,
};
use motion::Painter;

use super::{GAP, Strip};
use crate::sortable::{self, Sortable};

/// Persistent gesture and animation state, one per strip.
pub struct Reorder<Id>(Sortable<(), Id>);

impl<Id> Clone for Reorder<Id> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Id: Clone + PartialEq + 'static> Reorder<Id> {
    pub fn new(painter: Painter) -> Self {
        Self(Sortable::new(painter))
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
        let list = sortable::List::new("tabs", (), Axis::Horizontal, tabs).gap(px(GAP));
        Bar {
            group: self.0.group(id, [list]).axis_locked(),
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

#[derive(IntoElement)]
pub struct Bar<Id: Clone + PartialEq + 'static> {
    group: sortable::Group<(), Id>,
}

impl<Id: Clone + PartialEq + 'static> Bar<Id> {
    /// Apply the final move synchronously. Accepts `cx.listener`.
    pub fn on_reorder(mut self, moved: impl Fn(&Move, &mut Window, &mut App) + 'static) -> Self {
        self.group = self.group.on_drop(move |event, window, cx| {
            moved(
                &Move {
                    from: event.from.index,
                    to: event.to.index,
                },
                window,
                cx,
            );
        });
        self
    }

    pub fn on_drop_outside(
        mut self,
        outside: impl Fn(&OutsideDrop<Id>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.group = self.group.on_drop_outside(move |event, window, cx| {
            outside(
                &OutsideDrop {
                    id: event.item.clone(),
                    position: event.position,
                },
                window,
                cx,
            );
        });
        self
    }
}

impl<Id: Clone + PartialEq + 'static> RenderOnce for Bar<Id> {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        self.group
            .drag_style(StyleRefinement::default().bg(theme::Theme::of(cx).surface_raised))
    }
}
