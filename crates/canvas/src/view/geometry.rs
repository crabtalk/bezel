use crate::{
    edit::CanvasEditor,
    view::{Positions, edge_paths::EdgePaths},
};
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct Geometry {
    revision: Option<(u64, u64)>,
    held: Option<String>,
    pub positions: Positions,
    pub order: Vec<usize>,
    pub paths: EdgePaths,
}

impl Geometry {
    pub fn refresh(&mut self, editor: &CanvasEditor, held: Option<&str>) -> bool {
        let revision = editor.paint_revision();
        if self.revision == Some(revision) && self.held.as_deref() == held {
            return false;
        }
        let canvas = editor.painted();
        if self.revision != Some(revision) {
            self.positions.clear();
            self.positions.extend(
                canvas
                    .nodes
                    .iter()
                    .map(|node| (node.id.clone(), (node.x as f32, node.y as f32))),
            );
        }
        let containment = editor.painted_containment();
        let carried: HashSet<String> = held.map_or_else(HashSet::new, |id| {
            containment
                .with_contents(&[id.to_owned()])
                .into_iter()
                .collect()
        });
        self.order.clear();
        self.order.extend(0..canvas.nodes.len());
        self.order.sort_by_key(|&index| {
            let id = &canvas.nodes[index].id;
            (carried.contains(id), containment.depth(id))
        });
        self.revision = Some(revision);
        self.held = held.map(str::to_owned);
        true
    }
}

#[cfg(test)]
#[path = "../../tests/unit/geometry.rs"]
mod tests;
