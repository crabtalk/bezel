use crate::{
    kind::{Card, Look, Paint},
    model::Node,
};
use gpui::{AnyElement, App, Context, Render, SharedString, StyleRefinement, Window, prelude::*};
use std::rc::Rc;

struct Body {
    node: Node,
    card: Card,
    paint: Paint,
}

pub(super) fn render(
    node: &Node,
    card: Card,
    paint: Paint,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let key: SharedString = format!("canvas-body-{}", node.id).into();
    let mut node = node.clone();
    node.x = 0;
    node.y = 0;
    let body = window.use_keyed_state(key, cx, |_, _| Body {
        node: node.clone(),
        card: card.clone(),
        paint: paint.clone(),
    });
    let size = card.size;
    body.update(cx, |body, cx| {
        if body.node != node
            || body.card.zoom != card.zoom
            || body.card.size != card.size
            || body.card.typography != card.typography
            || body.card.selected != card.selected
            || body.card.sizing != card.sizing
            || !Rc::ptr_eq(&body.paint, &paint)
        {
            body.node = node;
            body.card = card;
            body.paint = paint;
            cx.notify();
        }
    });
    body.cached_movable(StyleRefinement::default().w(size.width).h(size.height))
        .into_any_element()
}

impl Render for Body {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        (self.paint)(
            &self.node,
            Look {
                card: self.card.clone(),
                editor: None,
            },
            window,
            cx,
        )
    }
}
