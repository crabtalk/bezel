//! Every edit the canvas makes, as data.
//!
//! The view turns keys, drags and typing into batches of [`Change`]s and hands
//! each to the app's filter before [`apply`]ing them, so an app can refuse one,
//! rewrite it, or do something of its own beside it. A batch lands whole or not
//! at all, and applying one answers the batch that undoes it. The changes are a
//! graph's; a tree edit in [`mindmap`](crate::mindmap) answers the batch it
//! makes.

use crate::model::{Canvas, Edge, Node};

// Handed on and dropped, never kept in bulk; boxing the node would only put a
// `Box::new` in every filter.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    /// At `index` among the nodes, on top when `None`.
    AddNode {
        node: Node,
        index: Option<usize>,
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
        Change::AddNode { node, .. } => Some(node.id.as_str()),
        _ => None,
    })
}

/// Apply a batch in order, answering the batch that undoes it.
pub fn apply_all(canvas: &mut Canvas, changes: &[Change]) -> Vec<Change> {
    let mut undo = Vec::new();
    for change in changes {
        undo.extend(apply(canvas, change).into_iter().rev());
    }
    // Reverse batch order while preserving each inverse batch's order.
    undo.reverse();
    undo
}

/// Apply `change`, answering the changes that undo it.
pub fn apply(canvas: &mut Canvas, change: &Change) -> Vec<Change> {
    match change {
        Change::AddNode { node, index } => {
            let len = canvas.nodes.len();
            canvas
                .nodes
                .insert(index.unwrap_or(len).min(len), node.clone());
            vec![Change::RemoveNodes {
                ids: vec![node.id.clone()],
            }]
        }
        Change::AddEdge { edge, index } => {
            let len = canvas.edges.len();
            canvas
                .edges
                .insert(index.unwrap_or(len).min(len), edge.clone());
            vec![Change::RemoveEdges {
                ids: vec![edge.id.clone()],
            }]
        }
        Change::RemoveNodes { ids } => {
            let gone = |id: &String| ids.contains(id);
            // Put back lowest index first, so each lands where it was.
            let mut undo: Vec<Change> = canvas
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| gone(&node.id))
                .map(|(ix, node)| Change::AddNode {
                    node: node.clone(),
                    index: Some(ix),
                })
                .collect();
            undo.extend(
                canvas
                    .edges
                    .iter()
                    .enumerate()
                    .filter(|(_, edge)| gone(&edge.from_node) || gone(&edge.to_node))
                    .map(|(ix, edge)| Change::AddEdge {
                        edge: edge.clone(),
                        index: Some(ix),
                    }),
            );
            canvas.nodes.retain(|node| !gone(&node.id));
            canvas
                .edges
                .retain(|edge| !gone(&edge.from_node) && !gone(&edge.to_node));
            undo
        }
        Change::RemoveEdges { ids } => {
            let undo = canvas
                .edges
                .iter()
                .enumerate()
                .filter(|(_, edge)| ids.contains(&edge.id))
                .map(|(ix, edge)| Change::AddEdge {
                    edge: edge.clone(),
                    index: Some(ix),
                })
                .collect();
            canvas.edges.retain(|edge| !ids.contains(&edge.id));
            undo
        }
        Change::MoveNodes { moves } => {
            let mut back = Vec::new();
            for (id, to) in moves {
                if let Some(node) = canvas.node_mut(id) {
                    back.push((id.clone(), (node.x, node.y)));
                    (node.x, node.y) = *to;
                }
            }
            // A node moved twice goes back to where it began.
            back.reverse();
            if back.is_empty() {
                Vec::new()
            } else {
                vec![Change::MoveNodes { moves: back }]
            }
        }
        Change::Resize { id, size } => match canvas.node_mut(id) {
            Some(node) => {
                let old = (node.width, node.height);
                (node.width, node.height) = *size;
                vec![Change::Resize {
                    id: id.clone(),
                    size: old,
                }]
            }
            None => Vec::new(),
        },
        Change::UpdateNode { node } => match canvas.node_mut(&node.id) {
            Some(old) => vec![Change::UpdateNode {
                node: std::mem::replace(old, node.clone()),
            }],
            None => Vec::new(),
        },
        Change::UpdateEdge { edge } => {
            match canvas.edges.iter_mut().find(|old| old.id == edge.id) {
                Some(old) => vec![Change::UpdateEdge {
                    edge: std::mem::replace(old, edge.clone()),
                }],
                None => Vec::new(),
            }
        }
    }
}
