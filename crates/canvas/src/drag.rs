//! What dragging a node does — the app's call, per view.
//!
//! ```ignore
//! cx.new(|cx| CanvasView::new(doc, cx).with_drag(canvas::drag::reparent))
//! ```
//!
//! A handler answers each move and the drop with [`Change`]s. On a move its
//! `MoveNodes` are applied as they come, and the rest are drawn as what the
//! drop would do — a ring on a node an added edge reaches, the connector it
//! would make, the ones removed edges would cut. The drop is one batch through
//! the view's change filter, with the preview put back first, so a refused
//! drop leaves everything where it was. [`pin`], [`reparent`] and [`detach`]
//! are answers, not the list: an app writes its own with the same signature.

use crate::{change::Change, mindmap, model::Canvas};

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
pub fn pin(canvas: &Canvas, drag: &Drag) -> Vec<Change> {
    mindmap::carry(canvas, drag.id, drag.to(), drag.phase == Phase::Drop)
}

/// Dropped on another node, becomes its last child; anywhere else, goes back.
pub fn reparent(canvas: &Canvas, drag: &Drag) -> Vec<Change> {
    let follow = mindmap::carry(canvas, drag.id, drag.to(), false);
    match drag
        .over
        .and_then(|parent| mindmap::reparent(canvas, drag.id, parent))
    {
        // The unpin replaces the whole node, so the move comes after it.
        Some(hang) => hang.into_iter().chain(follow).collect(),
        None if drag.phase == Phase::Move => follow,
        None => Vec::new(),
    }
}

/// Dropped anywhere, the branches into it are cut: a root of its own, where it
/// landed.
pub fn detach(canvas: &Canvas, drag: &Drag) -> Vec<Change> {
    let mut changes = mindmap::carry(canvas, drag.id, drag.to(), false);
    changes.extend(mindmap::detach(canvas, drag.id));
    changes
}
