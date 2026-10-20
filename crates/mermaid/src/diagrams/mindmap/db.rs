//! Pele's `diagrams/mindmap/db.ts`.

use serde::{Serialize, Serializer, ser::SerializeMap};

use crate::{
    Error, Kind,
    diagrams::common::outline::{NodeType, OutlineDb},
};

const SECTIONS: usize = 11;

#[derive(Clone, Debug, PartialEq)]
pub struct MindmapNode {
    pub id: usize,
    pub node_id: String,
    pub level: i64,
    pub descr: String,
    pub kind: NodeType,
    /// Indices into the model's `nodes`.
    pub children: Vec<usize>,
    pub is_root: bool,
    /// Index of the first-level branch the node belongs to, wrapping after
    /// eleven as in Mermaid.
    pub section: Option<usize>,
    pub class: Option<String>,
    pub icon: Option<String>,
}

/// Pele's `MindmapDb`, which is its model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MindmapModel {
    pub title: Option<String>,
    /// In source order, which is depth-first. The first node is the root.
    pub nodes: Vec<MindmapNode>,
    base_level: usize,
    /// The chain of nodes that can still take children, with strictly
    /// increasing levels.
    open: Vec<usize>,
}

impl MindmapModel {
    pub fn new(title: Option<&str>) -> Self {
        Self {
            title: title.map(str::to_string),
            ..Self::default()
        }
    }
}

impl OutlineDb for MindmapModel {
    fn add_node(
        &mut self,
        level: usize,
        id: &str,
        descr: &str,
        kind: NodeType,
        _shape_data: Option<&str>,
    ) -> Result<(), Error> {
        let is_root = self.nodes.is_empty();
        if is_root {
            self.base_level = level;
        }
        let level = level as i64 - self.base_level as i64;

        // Mermaid's parent is the last node with a smaller indentation,
        // wherever it is in the tree.
        let mut depth = self.open.len();
        while depth > 0 && self.nodes[self.open[depth - 1]].level >= level {
            depth -= 1;
        }
        let parent = depth.checked_sub(1).map(|at| self.open[at]);
        if parent.is_none() && !is_root {
            return Err(Error::semantic(format!(
                "There can be only one root. No parent could be found for (\"{descr}\")"
            ))
            .of(Kind::Mindmap));
        }
        let at = self.nodes.len();
        let mut node = MindmapNode {
            id: at,
            node_id: id.to_string(),
            level,
            descr: descr.to_string(),
            kind,
            children: Vec::new(),
            is_root,
            section: None,
            class: None,
            icon: None,
        };
        if let Some(parent) = parent {
            let parent = &mut self.nodes[parent];
            node.section = if parent.is_root {
                Some(parent.children.len() % SECTIONS)
            } else {
                parent.section
            };
            parent.children.push(at);
        }
        self.open.truncate(depth);
        self.open.push(at);
        self.nodes.push(node);
        Ok(())
    }

    /// Mermaid throws a TypeError when a decoration comes before any node.
    /// Here it is ignored.
    fn decorate_node(&mut self, icon: Option<&str>, class: Option<&str>) {
        let Some(node) = self.nodes.last_mut() else {
            return;
        };
        if let Some(icon) = icon.filter(|icon| !icon.is_empty()) {
            node.icon = Some(icon.to_string());
        }
        if let Some(class) = class.filter(|class| !class.is_empty()) {
            node.class = Some(class.to_string());
        }
    }
}

/// A node as `JSON.stringify` writes it, its children nested.
struct Nested<'m> {
    nodes: &'m [MindmapNode],
    at: usize,
}

impl Serialize for Nested<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let node = &self.nodes[self.at];
        let mut out = serializer.serialize_map(None)?;
        out.serialize_entry("id", &node.id)?;
        out.serialize_entry("nodeId", &node.node_id)?;
        out.serialize_entry("level", &node.level)?;
        out.serialize_entry("descr", &node.descr)?;
        out.serialize_entry("type", &node.kind)?;
        out.serialize_entry("children", &self.list(&node.children))?;
        out.serialize_entry("isRoot", &node.is_root)?;
        if let Some(section) = node.section {
            out.serialize_entry("section", &section)?;
        }
        if let Some(icon) = &node.icon {
            out.serialize_entry("icon", icon)?;
        }
        if let Some(class) = &node.class {
            out.serialize_entry("class", class)?;
        }
        out.end()
    }
}

impl<'m> Nested<'m> {
    fn list(&self, ids: &[usize]) -> Vec<Nested<'m>> {
        ids.iter()
            .map(|&at| Nested {
                nodes: self.nodes,
                at,
            })
            .collect()
    }
}

impl Serialize for MindmapModel {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let all = Nested {
            nodes: &self.nodes,
            at: 0,
        };
        let mut out = serializer.serialize_map(None)?;
        out.serialize_entry("type", "mindmap")?;
        if let Some(title) = &self.title {
            out.serialize_entry("title", title)?;
        }
        out.serialize_entry(
            "nodes",
            &all.list(&(0..self.nodes.len()).collect::<Vec<_>>()),
        )?;
        out.serialize_entry("baseLevel", &self.base_level)?;
        out.serialize_entry("open", &all.list(&self.open))?;
        out.end()
    }
}
