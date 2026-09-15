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
    /// The rest of the selection, carried along.
    pub with: &'a [String],
    /// What the held nodes hold: carried along, not otherwise moved.
    pub contents: &'a [String],
    /// Where the node was when the press began.
    pub origin: (i64, i64),
    /// How far the pointer has moved since, in canvas units.
    pub delta: (i64, i64),
    /// The node under the pointer, outside the dragged branches.
    pub over: Option<&'a str>,
    pub phase: Phase,
}

impl Drag<'_> {
    /// Where the pointer has carried the node.
    pub fn to(&self) -> (i64, i64) {
        (self.origin.0 + self.delta.0, self.origin.1 + self.delta.1)
    }

    /// The held node, then the rest of the selection.
    pub fn ids(&self) -> Vec<String> {
        std::iter::once(self.id.to_owned())
            .chain(self.with.iter().cloned())
            .collect()
    }
}

pub type DragHandler = fn(&Canvas, &Drag) -> Vec<Change>;

/// Stays where it is dropped, its branch following. The default.
pub fn pin(canvas: &Canvas, drag: &Drag) -> Vec<Change> {
    follow(canvas, drag, drag.phase == Phase::Drop)
}

/// Dropped on another node, becomes its last child; anywhere else, goes back.
pub fn reparent(canvas: &Canvas, drag: &Drag) -> Vec<Change> {
    let follow = follow(canvas, drag, false);
    match drag
        .over
        .and_then(|parent| mindmap::reparent(canvas, &drag.ids(), parent))
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
    let mut changes = follow(canvas, drag, false);
    changes.extend(mindmap::detach(canvas, &drag.ids()));
    changes
}

/// The selection and what it holds moved as far as the pointer carried the
/// held node.
fn follow(canvas: &Canvas, drag: &Drag, pin: bool) -> Vec<Change> {
    let Some(node) = canvas.node(drag.id) else {
        return Vec::new();
    };
    let to = drag.to();
    let mut ids = drag.ids();
    ids.extend(drag.contents.iter().cloned());
    mindmap::carry(canvas, &ids, (to.0 - node.x, to.1 - node.y), pin)
}
