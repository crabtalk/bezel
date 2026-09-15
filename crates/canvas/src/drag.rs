//! What dragging a node does — the app's call, per view.
//!
//! ```ignore
//! cx.new(|cx| CanvasView::new(doc, cx).with_drag(canvas::drag::reparent))
//! ```
//!
//! A handler runs on every move and once on the drop, with the document to
//! change. While a node is held, layout keeps it where the handler put it and
//! its branch follows. [`pin`], [`reparent`] and [`detach`] are answers, not
//! the list: an app writes its own with the same signature.

use crate::{mindmap, model::Canvas};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Move,
    Drop,
}

#[derive(Clone, Copy, Debug)]
pub struct Drag<'a> {
    pub id: &'a str,
    /// Where the node was when the press began.
    pub origin: (i64, i64),
    /// How far the pointer has moved since, in canvas units.
    pub delta: (i64, i64),
    /// The node under the pointer, outside the dragged branch.
    pub over: Option<&'a str>,
    pub phase: Phase,
}

impl Drag<'_> {
    /// Where the pointer has carried the node.
    pub fn to(&self) -> (i64, i64) {
        (self.origin.0 + self.delta.0, self.origin.1 + self.delta.1)
    }
}

pub type DragHandler = fn(&mut Canvas, &Drag);

/// Stays where it is dropped, its branch following. The default.
pub fn pin(canvas: &mut Canvas, drag: &Drag) {
    let (x, y) = drag.to();
    mindmap::pin(canvas, drag.id, x, y);
}

/// Dropped on another node, becomes its last child; anywhere else, goes back.
pub fn reparent(canvas: &mut Canvas, drag: &Drag) {
    match (drag.phase, drag.over) {
        (Phase::Move, _) => follow(canvas, drag),
        (Phase::Drop, Some(parent)) => {
            mindmap::reparent(canvas, drag.id, parent);
        }
        (Phase::Drop, None) => {}
    }
}

/// Dropped anywhere, the edges into it are cut: a root of its own, where it
/// landed.
pub fn detach(canvas: &mut Canvas, drag: &Drag) {
    follow(canvas, drag);
    if drag.phase == Phase::Drop {
        mindmap::detach(canvas, drag.id);
    }
}

fn follow(canvas: &mut Canvas, drag: &Drag) {
    if let Some(node) = canvas.node_mut(drag.id) {
        (node.x, node.y) = drag.to();
    }
}
