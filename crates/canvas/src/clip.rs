//! What copy and paste carry: a fragment of a canvas, as a canvas of its own.
//!
//! The clipboard holds it as JSON Canvas, so a copy pastes into other canvas
//! apps and back.

use std::collections::HashMap;

use crate::{
    change::Change,
    mindmap,
    model::{Canvas, Edge},
};

/// The nodes `ids` name, in paint order, and the edges between two of them.
pub fn fragment(canvas: &Canvas, ids: &[String]) -> Canvas {
    let keep = |id: &String| ids.contains(id);
    Canvas {
        nodes: canvas
            .nodes
            .iter()
            .filter(|node| keep(&node.id))
            .cloned()
            .collect(),
        edges: canvas
            .edges
            .iter()
            .filter(|edge| keep(&edge.from_node) && keep(&edge.to_node))
            .cloned()
            .collect(),
        ..Canvas::default()
    }
}

/// The top left of a fragment's nodes, and the size they take.
pub fn bounds(fragment: &Canvas) -> Option<((i64, i64), (i64, i64))> {
    let first = fragment.nodes.first()?;
    let start = (
        first.x,
        first.y,
        first.x + first.width,
        first.y + first.height,
    );
    let (x0, y0, x1, y1) = fragment.nodes.iter().fold(start, |(x0, y0, x1, y1), n| {
        (
            x0.min(n.x),
            y0.min(n.y),
            x1.max(n.x + n.width),
            y1.max(n.y + n.height),
        )
    });
    Some(((x0, y0), (x1 - x0, y1 - y0)))
}

/// `fragment` added to `canvas` under fresh ids, its top left at `at`, and its
/// roots hung under `under` when given.
pub fn paste(
    canvas: &Canvas,
    fragment: &Canvas,
    at: (i64, i64),
    under: Option<&str>,
) -> Vec<Change> {
    let Some(((x0, y0), _)) = bounds(fragment) else {
        return Vec::new();
    };
    let roots: Vec<&str> = match under {
        Some(_) => mindmap::roots(fragment)
            .map(|node| node.id.as_str())
            .collect(),
        None => Vec::new(),
    };
    let wanted = fragment.nodes.len() + fragment.edges.len() + roots.len();
    let mut fresh = canvas.mint_n(wanted).into_iter();
    let ids: HashMap<&str, String> = fragment
        .nodes
        .iter()
        .zip(fresh.by_ref())
        .map(|(node, id)| (node.id.as_str(), id))
        .collect();

    let mut changes: Vec<Change> = fragment
        .nodes
        .iter()
        .map(|old| {
            let mut node = old.clone();
            node.id = ids[old.id.as_str()].clone();
            (node.x, node.y) = (old.x - x0 + at.0, old.y - y0 + at.1);
            if roots.contains(&old.id.as_str()) {
                node.extra.remove(mindmap::PINNED);
            }
            Change::AddNode { node, index: None }
        })
        .collect();
    for edge in &fragment.edges {
        let from = ids.get(edge.from_node.as_str());
        let to = ids.get(edge.to_node.as_str());
        if let (Some(from), Some(to), Some(id)) = (from, to, fresh.next()) {
            let edge = Edge {
                id,
                from_node: from.clone(),
                to_node: to.clone(),
                ..edge.clone()
            };
            changes.push(Change::AddEdge { edge, index: None });
        }
    }
    if let Some(parent) = under {
        for (root, id) in roots.into_iter().zip(fresh) {
            let edge = mindmap::branch_edge(id, parent, &ids[root]);
            changes.push(Change::AddEdge { edge, index: None });
        }
    }
    changes
}
