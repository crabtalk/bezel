//! Every edit the canvas makes, as data.
//!
//! The view turns keys, drags and typing into [`Change`]s and hands each to the
//! app's filter before [`apply`]ing it, so an app can refuse one, rewrite it,
//! or do something of its own beside it. An app changing the document itself
//! sends the same values.

use crate::{
    mindmap,
    model::{Canvas, Edge, Node},
};

// Handed on and dropped, never kept in bulk; boxing the node would only put a
// `Box::new` in every filter.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    /// A node, and the edge hanging it off another at `index` among the edges.
    Add {
        node: Node,
        edge: Option<Edge>,
        index: Option<usize>,
    },
    /// Put a node at `to`; `pin` keeps it there through layout.
    Move {
        id: String,
        to: (i64, i64),
        pin: bool,
    },
    /// Hang `id` under `parent`, cutting its edges in.
    Reparent { id: String, parent: String },
    /// Cut the edges into `id`.
    Detach { id: String },
    /// A node and its branch.
    Remove { id: String },
    /// A node replaced by one with the same id — what editing in place sends.
    Update { node: Node },
}

impl Change {
    /// The node the change is about.
    pub fn id(&self) -> &str {
        match self {
            Self::Add { node, .. } | Self::Update { node } => &node.id,
            Self::Move { id, .. }
            | Self::Reparent { id, .. }
            | Self::Detach { id }
            | Self::Remove { id } => id,
        }
    }
}

pub fn apply(canvas: &mut Canvas, change: &Change) {
    match change {
        Change::Add { node, edge, index } => {
            canvas.nodes.push(node.clone());
            if let Some(edge) = edge {
                let len = canvas.edges.len();
                canvas
                    .edges
                    .insert(index.unwrap_or(len).min(len), edge.clone());
            }
        }
        Change::Move { id, to, pin: true } => {
            mindmap::pin(canvas, id, to.0, to.1);
        }
        Change::Move { id, to, pin: false } => {
            if let Some(node) = canvas.node_mut(id) {
                (node.x, node.y) = *to;
            }
        }
        Change::Reparent { id, parent } => {
            mindmap::reparent(canvas, id, parent);
        }
        Change::Detach { id } => mindmap::detach(canvas, id),
        Change::Remove { id } => {
            mindmap::remove(canvas, id);
        }
        Change::Update { node } => {
            if let Some(old) = canvas.node_mut(&node.id) {
                *old = node.clone();
            }
        }
    }
}
