//! What dragging a node does — the app's call, per view.
//!
//! ```ignore
//! cx.new(|cx| CanvasView::new(doc, cx).with_drag(canvas::drag::reparent))
//! ```
//!
//! A handler answers each move and the drop with [`Change`]s. On a move its
//! `Move`s are applied as they come, and the rest are drawn as what the drop
//! would do — a ring on a new parent, the connector it would make, the ones it
//! would cut. The drop's go through the view's change filter, with the held
//! node put back first, so a refused drop leaves it where it was. [`pin`],
//! [`reparent`] and [`detach`] are answers, not the list: an app writes its own
//! with the same signature.

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
    let Some(parent) = drag.over else {
        return match drag.phase {
            Phase::Move => vec![follow(drag)],
            Phase::Drop => Vec::new(),
        };
    };
    vec![
        follow(drag),
        Change::Reparent {
            id: drag.id.into(),
            parent: parent.into(),
        },
    ]
}

/// Dropped anywhere, the edges into it are cut: a root of its own, where it
/// landed.
pub fn detach(_: &Canvas, drag: &Drag) -> Vec<Change> {
    vec![follow(drag), Change::Detach { id: drag.id.into() }]
}

fn follow(drag: &Drag) -> Change {
    Change::Move {
        id: drag.id.into(),
        to: drag.to(),
        pin: false,
    }
}
