//! A group is a frame: the nodes inside its box are its members, and go where
//! it goes.

use crate::model::{Canvas, GROUP, Node};

/// Whether `inner`'s box lies within `outer`'s.
pub fn within(inner: &Node, outer: &Node) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

/// The nodes inside group `id`, nested groups and their members included.
pub fn members(canvas: &Canvas, id: &str) -> Vec<String> {
    let Some(group) = canvas.node(id).filter(|node| node.kind == GROUP) else {
        return Vec::new();
    };
    canvas
        .nodes
        .iter()
        .filter(|node| node.id != id && within(node, group))
        .map(|node| node.id.clone())
        .collect()
}

/// `ids`, each group followed by its members, none twice.
pub fn with_members(canvas: &Canvas, ids: &[String]) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    for id in ids {
        for id in std::iter::once(id.clone()).chain(members(canvas, id)) {
            if !all.contains(&id) {
                all.push(id);
            }
        }
    }
    all
}
