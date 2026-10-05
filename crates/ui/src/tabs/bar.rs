//! The shared scroll surface for plain and reorderable tab strips.
//!
//! The strip scrolls by wheel and trackpad only; it draws no scrollbar.

use gpui::{
    AnyElement, App, Div, ElementId, Interactivity, IntoElement, Refineable, RenderOnce,
    ScrollHandle, Stateful, StyleRefinement, Window, div, prelude::*, px,
};

#[derive(IntoElement)]
pub struct TabBar {
    id: ElementId,
    row: Stateful<Div>,
    viewport: Option<AnyElement>,
    handle: Option<ScrollHandle>,
    style: StyleRefinement,
}

impl TabBar {
    pub(super) fn new(id: ElementId) -> Self {
        Self {
            id,
            row: div()
                .id("tab-row")
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(super::GAP))
                .overflow_x_scroll(),
            viewport: None,
            handle: None,
            style: StyleRefinement::default(),
        }
    }

    /// Optional access for programmatic scrolling. Otherwise the bar keeps its own handle.
    pub fn track_scroll(mut self, handle: &ScrollHandle) -> Self {
        self.handle = Some(handle.clone());
        self
    }

    pub(super) fn viewport(mut self, viewport: AnyElement, handle: &ScrollHandle) -> Self {
        self.viewport = Some(viewport);
        self.track_scroll(handle)
    }
}

impl ParentElement for TabBar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.row.extend(elements);
    }
}

impl Styled for TabBar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl InteractiveElement for TabBar {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.row.interactivity()
    }
}

impl StatefulInteractiveElement for TabBar {}

impl RenderOnce for TabBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let handle = self.handle.unwrap_or_else(|| {
            window
                .use_keyed_state(self.id.clone(), cx, |_, _| ScrollHandle::new())
                .read(cx)
                .clone()
        });
        let viewport = self
            .viewport
            .unwrap_or_else(|| self.row.flex_1().track_scroll(&handle).into_any_element());
        let mut frame = div()
            .id(self.id)
            .relative()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_1();
        frame.style().refine(&self.style);
        frame.child(viewport)
    }
}
