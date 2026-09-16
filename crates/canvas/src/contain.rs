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

/// Containment shared by hit testing, carrying and paint order.
pub(crate) struct Index {
    held: HashMap<String, Vec<String>>,
    depths: HashMap<String, usize>,
}

impl Index {
    pub(crate) fn new(canvas: &Canvas, holds: impl Fn(&Node) -> bool) -> Self {
        let parents = containers(canvas, holds);
        let mut held: HashMap<String, Vec<String>> = HashMap::new();
        for (&id, &by) in &parents {
            held.entry(by.to_owned()).or_default().push(id.to_owned());
        }
        let mut depths = HashMap::with_capacity(canvas.nodes.len());
        let mut path = Vec::new();
        let mut seen = HashMap::new();
        for node in &canvas.nodes {
            path.clear();
            seen.clear();
            let mut at = node.id.as_str();
            let mut depth = loop {
                if let Some(&depth) = depths.get(at) {
                    break depth;
                }
                if let Some(&start) = seen.get(at) {
                    // Each cycle member counts every other member once.
                    let depth = path.len() - start - 1;
                    for id in path.drain(start..) {
                        depths.insert(str::to_owned(id), depth);
                    }
                    break depth;
                }
                let Some(&parent) = parents.get(at) else {
                    depths.insert(at.to_owned(), 0);
                    break 0;
                };
                seen.insert(at, path.len());
                path.push(at);
                at = parent;
            };
            for id in path.drain(..).rev() {
                depth += 1;
                depths.insert(id.to_owned(), depth);
            }
        }
        Self { held, depths }
    }

    pub(crate) fn depth(&self, id: &str) -> usize {
        self.depths.get(id).copied().unwrap_or(0)
    }

    pub(crate) fn with_contents(&self, ids: &[String]) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut all: Vec<&str> = ids
            .iter()
            .map(String::as_str)
            .filter(|id| seen.insert(*id))
            .collect();
        let mut at = 0;
        while at < all.len() {
            for inner in self.held.get(all[at]).into_iter().flatten() {
                if seen.insert(inner.as_str()) {
                    all.push(inner);
                }
            }
            at += 1;
        }
        all.into_iter().map(str::to_owned).collect()
    }

    pub(crate) fn topmost<'a>(
        &self,
        canvas: &'a Canvas,
        at: (i64, i64),
        except: &[String],
        accept: impl Fn(&Node) -> bool,
    ) -> Option<&'a str> {
        let except: HashSet<&str> = except.iter().map(String::as_str).collect();
        canvas
            .nodes
            .iter()
            .filter(|node| {
                !except.contains(node.id.as_str())
                    && accept(node)
                    && (node.x..node.x + node.width).contains(&at.0)
                    && (node.y..node.y + node.height).contains(&at.1)
            })
            .max_by_key(|node| self.depth(&node.id))
            .map(|node| node.id.as_str())
    }
}

/// `ids`, then everything they hold however deep, none twice.
pub fn with_contents(
    canvas: &Canvas,
    ids: &[String],
    holds: impl Fn(&Node) -> bool,
) -> Vec<String> {
    Index::new(canvas, holds).with_contents(ids)
}

/// How many containers hold each node: shallower paints first.
pub fn depths(canvas: &Canvas, holds: impl Fn(&Node) -> bool) -> HashMap<String, usize> {
    Index::new(canvas, holds).depths
}

/// The node at `at` that `accept` takes, outside `except`: what a container
/// holds before the container, and the topmost of those.
pub fn topmost<'a>(
    canvas: &'a Canvas,
    at: (i64, i64),
    except: &[String],
    accept: impl Fn(&Node) -> bool,
    holds: impl Fn(&Node) -> bool,
) -> Option<&'a str> {
    Index::new(canvas, holds).topmost(canvas, at, except, accept)
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
