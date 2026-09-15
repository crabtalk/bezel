//! What dragging a node does — the app's call, per view.
//!
//! ```ignore
//! cx.new(|cx| CanvasView::new(doc, cx).with_drag(canvas::drag::reparent))
//! ```
//!
//! A handler answers each move and the drop with [`Change`]s. A move's are a
//! preview, applied as they come; the drop's go through the view's change
//! filter, and the held node is put back first, so a refused drop leaves it
//! where it was. While a node is held, layout keeps it where the preview put
//! it and its branch follows. [`pin`], [`reparent`] and [`detach`] are answers,
//! not the list: an app writes its own with the same signature.

use crate::{change::Change, model::Canvas};

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

pub type DragHandler = fn(&Canvas, &Drag) -> Vec<Change>;

/// Stays where it is dropped, its branch following. The default.
pub fn pin(_: &Canvas, drag: &Drag) -> Vec<Change> {
    vec![Change::Move {
        id: drag.id.into(),
        to: drag.to(),
        pin: true,
    }]
}

/// Dropped on another node, becomes its last child; anywhere else, goes back.
pub fn reparent(_: &Canvas, drag: &Drag) -> Vec<Change> {
    match (drag.phase, drag.over) {
        (Phase::Move, _) => vec![follow(drag)],
        (Phase::Drop, Some(parent)) => vec![Change::Reparent {
            id: drag.id.into(),
            parent: parent.into(),
        }],
        (Phase::Drop, None) => Vec::new(),
    }
}

/// Dropped anywhere, the edges into it are cut: a root of its own, where it
/// landed.
pub fn detach(_: &Canvas, drag: &Drag) -> Vec<Change> {
    let mut changes = vec![follow(drag)];
    if drag.phase == Phase::Drop {
        changes.push(Change::Detach { id: drag.id.into() });
    }
    changes
}

fn follow(drag: &Drag) -> Change {
    Change::Move {
        id: drag.id.into(),
        to: drag.to(),
        pin: false,
    }
}
