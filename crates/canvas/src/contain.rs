//! Any node can hold others. A node names what holds it in our own
//! `"container"` field; one that names nothing sits in the smallest node
//! around it whose kind holds — a spec group, or an app's own. What a node
//! holds, however deep, goes where it goes and paints above it. Which nodes
//! hold is the caller's to say.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::{
    change::Change,
    model::{Canvas, Node},
};

/// Our own node field: the id of the node holding it.
pub const CONTAINER: &str = "container";

/// The container `node` names, if any.
pub fn named(node: &Node) -> Option<&str> {
    node.extra.get(CONTAINER).and_then(Value::as_str)
}

/// Whether `inner`'s box lies within `outer`'s.
pub fn within(inner: &Node, outer: &Node) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

/// What holds each held node, by id: the container it names, else the
/// smallest node around it that `holds`.
pub fn containers(canvas: &Canvas, holds: impl Fn(&Node) -> bool) -> HashMap<&str, &str> {
    let ids: HashSet<&str> = canvas.nodes.iter().map(|node| node.id.as_str()).collect();
    let holders: Vec<&Node> = canvas.nodes.iter().filter(|node| holds(node)).collect();
    canvas
        .nodes
        .iter()
        .filter_map(|node| {
            let area = node.width * node.height;
            let by = match named(node) {
                Some(id) => ids.get(id).copied().filter(|id| *id != node.id),
                None => holders
                    .iter()
                    .filter(|holder| holder.width * holder.height > area && within(node, holder))
                    .min_by_key(|holder| holder.width * holder.height)
                    .map(|holder| holder.id.as_str()),
            }?;
            Some((node.id.as_str(), by))
        })
        .collect()
}

/// `ids`, then everything they hold however deep, none twice.
pub fn with_contents(
    canvas: &Canvas,
    ids: &[String],
    holds: impl Fn(&Node) -> bool,
) -> Vec<String> {
    let mut held: HashMap<&str, Vec<&str>> = HashMap::new();
    for (id, by) in containers(canvas, holds) {
        held.entry(by).or_default().push(id);
    }
    let mut seen = HashSet::new();
    let mut all: Vec<String> = ids
        .iter()
        .filter(|id| seen.insert((*id).clone()))
        .cloned()
        .collect();
    let mut at = 0;
    while at < all.len() {
        let id = all[at].clone();
        for inner in held.get(id.as_str()).into_iter().flatten() {
            if seen.insert(inner.to_string()) {
                all.push(inner.to_string());
            }
        }
        at += 1;
    }
    all
}

/// How many containers hold each node: shallower paints first.
pub fn depths(canvas: &Canvas, holds: impl Fn(&Node) -> bool) -> HashMap<String, usize> {
    let containers = containers(canvas, holds);
    canvas
        .nodes
        .iter()
        .map(|node| {
            let (mut depth, mut at) = (0, node.id.as_str());
            let mut seen = HashSet::from([at]);
            while let Some(&by) = containers.get(at) {
                if !seen.insert(by) {
                    break;
                }
                (depth, at) = (depth + 1, by);
            }
            (node.id.clone(), depth)
        })
        .collect()
}

/// `id` naming `container` as what holds it, or naming nothing.
pub fn hold(canvas: &Canvas, id: &str, container: Option<&str>) -> Option<Change> {
    let mut node = canvas.node(id)?.clone();
    match container {
        Some(container) => node.extra.insert(CONTAINER.into(), container.into()),
        None => node.extra.remove(CONTAINER),
    };
    Some(Change::UpdateNode { node })
}

/// For `ids` where `canvas` has put them: each carried out of the container it
/// names lets it go.
pub fn loosen(canvas: &Canvas, ids: &[String]) -> Vec<Change> {
    ids.iter()
        .filter_map(|id| {
            let node = canvas.node(id)?;
            let container = canvas.node(named(node)?)?;
            if within(node, container) {
                return None;
            }
            hold(canvas, id, None)
        })
        .collect()
}
