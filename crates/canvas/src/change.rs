//! Every edit the canvas makes, as data.
//!
//! The view turns keys, drags and typing into batches of [`Change`]s and hands
//! each to the app's filter before [`apply`]ing them, so an app can refuse one,
//! rewrite it, or do something of its own beside it. A batch lands whole or not
//! at all, and applying one answers the batch that undoes it. The changes are a
//! graph's; a tree edit in [`mindmap`](crate::mindmap) answers the batch it
//! makes.

use crate::model::{Canvas, Edge, Node};
use std::collections::{HashMap, HashSet};

// Handed on and dropped, never kept in bulk; boxing the node would only put a
// `Box::new` in every filter.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    OrderNodes {
        ids: Vec<String>,
    },
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
    /// Drops the [`Edge::points`] of every edge touching a moved node.
    MoveNodes {
        moves: Vec<(String, (i64, i64))>,
    },
    /// A node's box. The view sends its measured heights unfiltered. Drops
    /// the [`Edge::points`] of every edge touching it.
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

/// Clear the route of every edge touching `ids`, answering what puts them
/// back.
fn unroute(canvas: &mut Canvas, includes: impl Fn(&str) -> bool) -> Vec<Change> {
    canvas
        .edges
        .iter_mut()
        .filter(|edge| {
            !edge.points.is_empty() && (includes(&edge.from_node) || includes(&edge.to_node))
        })
        .map(|edge| {
            let old = edge.clone();
            edge.points.clear();
            Change::UpdateEdge { edge: old }
        })
        .collect()
}

/// Apply `change`, answering the changes that undo it.
pub fn apply(canvas: &mut Canvas, change: &Change) -> Vec<Change> {
    match change {
        Change::OrderNodes { ids } => {
            let old: Vec<String> = canvas.nodes.iter().map(|node| node.id.clone()).collect();
            let mut order = HashMap::with_capacity(ids.len());
            for (index, id) in ids.iter().enumerate() {
                order.entry(id.as_str()).or_insert(index);
            }
            canvas
                .nodes
                .sort_by_key(|node| order.get(node.id.as_str()).copied().unwrap_or(usize::MAX));
            if canvas.nodes.iter().map(|node| &node.id).eq(old.iter()) {
                Vec::new()
            } else {
                vec![Change::OrderNodes { ids: old }]
            }
        }

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
            let ids: HashSet<&str> = ids.iter().map(String::as_str).collect();
            let gone = |id: &str| ids.contains(id);
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
            let ids: HashSet<&str> = ids.iter().map(String::as_str).collect();
            let undo = canvas
                .edges
                .iter()
                .enumerate()
                .filter(|(_, edge)| ids.contains(edge.id.as_str()))
                .map(|(ix, edge)| Change::AddEdge {
                    edge: edge.clone(),
                    index: Some(ix),
                })
                .collect();
            canvas.edges.retain(|edge| !ids.contains(edge.id.as_str()));
            undo
        }
        Change::MoveNodes { moves } => {
            let indices = (moves.len() > 1).then(|| {
                let indices = canvas.node_indices();
                moves
                    .iter()
                    .map(|(id, _)| indices.get(id.as_str()).copied())
                    .collect::<Vec<_>>()
            });
            let mut back = Vec::with_capacity(moves.len());
            for (offset, (id, to)) in moves.iter().enumerate() {
                let node = match &indices {
                    Some(indices) => indices[offset].map(|index| &mut canvas.nodes[index]),
                    None => canvas.node_mut(id),
                };
                if let Some(node) = node {
                    back.push((id.clone(), (node.x, node.y)));
                    (node.x, node.y) = *to;
                }
            }
            // A node moved twice goes back to where it began.
            back.reverse();
            if back.is_empty() {
                return Vec::new();
            }
            let mut undo = vec![Change::MoveNodes { moves: back }];
            if let [(id, _)] = moves.as_slice() {
                undo.extend(unroute(canvas, |candidate| candidate == id));
            } else {
                let moved: HashSet<&str> = moves.iter().map(|(id, _)| id.as_str()).collect();
                undo.extend(unroute(canvas, |id| moved.contains(id)));
            }
            undo
        }
        Change::Resize { id, size } => match canvas.node_mut(id) {
            Some(node) => {
                let old = (node.width, node.height);
                (node.width, node.height) = *size;
                let mut undo = vec![Change::Resize {
                    id: id.clone(),
                    size: old,
                }];
                if old != *size {
                    undo.extend(unroute(canvas, |candidate| candidate == id));
                }
                undo
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
