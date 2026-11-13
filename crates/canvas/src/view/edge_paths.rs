use crate::{
    change::Change,
    edit::CanvasEditor,
    path::{self, Ends, Path},
    view::{Positions, paint},
};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

struct Cached {
    ends: Ends,
    points: Vec<[i64; 2]>,
    kind: Option<String>,
    path: Rc<Path>,
}

#[derive(Default)]
pub(super) struct EdgePaths {
    paths: HashMap<String, Cached>,
    kinds_revision: u64,
}

impl EdgePaths {
    pub fn get(&self, id: &str) -> Option<&Rc<Path>> {
        self.paths.get(id).map(|cached| &cached.path)
    }

    pub fn refresh(&mut self, editor: &CanvasEditor, shown: &Positions) {
        if self.kinds_revision != editor.edge_kinds_revision {
            self.paths.clear();
            self.kinds_revision = editor.edge_kinds_revision;
        }
        let canvas = editor.painted();
        let nodes = canvas.lookup();
        let edges = canvas
            .edges
            .iter()
            .chain(editor.pending.iter().filter_map(|change| {
                if let Change::AddEdge { edge, .. } = change {
                    Some(edge)
                } else {
                    None
                }
            }));
        let mut live = HashSet::new();
        for edge in edges {
            let Some(ends) = paint::ends_of(&nodes, shown, edge, editor.kinds()) else {
                continue;
            };
            let settled = |id: &str| {
                let node = nodes.get(id);
                shown.get(id).is_none_or(|&(x, y)| {
                    node.is_some_and(|node| (x, y) == (node.x as f32, node.y as f32))
                })
            };
            let points = if settled(&edge.from_node) && settled(&edge.to_node) {
                edge.points.as_slice()
            } else {
                &[]
            };
            let draw = editor.edge_kinds().get(edge).path;
            live.insert(edge.id.as_str());
            if self.paths.get(&edge.id).is_some_and(|cached| {
                cached.ends == ends && cached.points == points && cached.kind == edge.kind
            }) {
                continue;
            }
            let path = path::route(&ends, points).unwrap_or_else(|| draw(&ends));
            self.paths.insert(
                edge.id.clone(),
                Cached {
                    ends,
                    points: points.to_vec(),
                    kind: edge.kind.clone(),
                    path: Rc::new(path),
                },
            );
        }
        self.paths.retain(|id, _| live.contains(id.as_str()));
    }
}

#[cfg(test)]
#[path = "../../tests/unit/edge_paths.rs"]
mod tests;
