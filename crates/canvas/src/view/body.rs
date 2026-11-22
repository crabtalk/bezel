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
    let copy = || Node {
        x: 0,
        y: 0,
        ..node.clone()
    };
    let body = window.use_keyed_state(key, cx, |_, _| Body {
        node: copy(),
        card: card.clone(),
        paint: paint.clone(),
    });
    let size = card.size;
    let current = body.read(cx);
    let node_changed = current.node.content() != node.content();
    if node_changed
        || current.card.zoom != card.zoom
        || current.card.size != card.size
        || current.card.typography != card.typography
        || current.card.selected != card.selected
        || current.card.sizing != card.sizing
        || !Rc::ptr_eq(&current.paint, &paint)
    {
        body.update(cx, |body, cx| {
            if node_changed {
                body.node = copy();
            }
            body.card = card;
            body.paint = paint;
            cx.notify();
        });
    }
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
