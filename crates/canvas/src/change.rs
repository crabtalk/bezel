//! Every edit the canvas makes, as data.
//!
//! The view turns keys, drags and typing into batches of [`Change`]s and hands
//! each to the app's filter before [`apply`]ing them, so an app can refuse one,
//! rewrite it, or do something of its own beside it. A batch lands whole or not
//! at all. The changes are a graph's; a tree edit in [`mindmap`](crate::mindmap)
//! answers the batch it makes.

use crate::model::{Canvas, Edge, Node};

// Handed on and dropped, never kept in bulk; boxing the node would only put a
// `Box::new` in every filter.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    /// On top of the others.
    AddNode {
        node: Node,
    },
    /// At `index` among the edges, the last when `None`. Edge order is a
    /// tree's child order.
    AddEdge {
        edge: Edge,
        index: Option<usize>,
    },
    /// Nodes, and every edge touching them.
    RemoveNodes {
        ids: Vec<String>,
    },
    RemoveEdges {
        ids: Vec<String>,
    },
    MoveNodes {
        moves: Vec<(String, (i64, i64))>,
    },
    /// A node's box. The view sends its measured heights unfiltered.
    Resize {
        id: String,
        size: (i64, i64),
    },
    /// The whole node, by id — so a batch moves it after, not before.
    UpdateNode {
        node: Node,
    },
    /// The whole edge, by id.
    UpdateEdge {
        edge: Edge,
    },
}

/// The first node a batch adds: what adding selects.
pub fn added(changes: &[Change]) -> Option<&str> {
    changes.iter().find_map(|change| match change {
        Change::AddNode { node } => Some(node.id.as_str()),
        _ => None,
    })
}

pub fn apply(canvas: &mut Canvas, change: &Change) {
    match change {
        Change::AddNode { node } => canvas.nodes.push(node.clone()),
        Change::AddEdge { edge, index } => {
            let len = canvas.edges.len();
            canvas
                .edges
                .insert(index.unwrap_or(len).min(len), edge.clone());
        }
        Change::RemoveNodes { ids } => {
            let gone = |id: &String| ids.contains(id);
            canvas.nodes.retain(|node| !gone(&node.id));
            canvas
                .edges
                .retain(|edge| !gone(&edge.from_node) && !gone(&edge.to_node));
        }
        Change::RemoveEdges { ids } => canvas.edges.retain(|edge| !ids.contains(&edge.id)),
        Change::MoveNodes { moves } => {
            for (id, to) in moves {
                if let Some(node) = canvas.node_mut(id) {
                    (node.x, node.y) = *to;
                }
            }
        }
        Change::Resize { id, size } => {
            if let Some(node) = canvas.node_mut(id) {
                (node.width, node.height) = *size;
            }
        }
        Change::UpdateNode { node } => {
            if let Some(old) = canvas.node_mut(&node.id) {
                *old = node.clone();
            }
        }
        Change::UpdateEdge { edge } => {
            if let Some(old) = canvas.edges.iter_mut().find(|old| old.id == edge.id) {
                *old = edge.clone();
            }
        }
    }
}
