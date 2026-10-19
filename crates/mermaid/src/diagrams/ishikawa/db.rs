//! Pele's `diagrams/ishikawa/db.ts`.

use serde::{Serialize, Serializer, ser::SerializeMap};

use super::types::{NodeId, Nodes, Tree};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IshikawaDb {
    pub title: Option<String>,
    pub root: Option<NodeId>,
    /// What the outline is made of.
    pub nodes: Nodes,
    /// The line of ancestors of the last node, each with its level.
    stack: Vec<(i64, NodeId)>,
    base_level: Option<i64>,
}

impl IshikawaDb {
    /// The first line is the effect. Every later line is a cause, nested by
    /// its indentation relative to the first cause, so the effect itself may
    /// be indented more than the causes.
    pub fn add_node(&mut self, raw_level: i64, text: String) {
        let node = self.nodes.add(text);
        if self.root.is_none() {
            self.root = Some(node);
            self.stack = vec![(0, node)];
            return;
        }
        let base = *self.base_level.get_or_insert(raw_level);
        let level = (raw_level - base + 1).max(1);
        while self.stack.len() > 1 && self.stack[self.stack.len() - 1].0 >= level {
            self.stack.pop();
        }
        let parent = self.stack[self.stack.len() - 1].1;
        self.nodes.0[parent].children.push(node);
        self.stack.push((level, node));
    }
}

impl Serialize for IshikawaDb {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        struct Frame<'a>(&'a Nodes, i64, NodeId);
        impl Serialize for Frame<'_> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let mut out = s.serialize_map(Some(2))?;
                out.serialize_entry("level", &self.1)?;
                out.serialize_entry(
                    "node",
                    &Tree {
                        nodes: self.0,
                        id: self.2,
                    },
                )?;
                out.end()
            }
        }
        let mut out = s.serialize_map(None)?;
        out.serialize_entry("type", "ishikawa")?;
        if let Some(title) = &self.title {
            out.serialize_entry("title", title)?;
        }
        if let Some(root) = self.root {
            out.serialize_entry(
                "root",
                &Tree {
                    nodes: &self.nodes,
                    id: root,
                },
            )?;
        }
        let stack: Vec<Frame> = self
            .stack
            .iter()
            .map(|&(level, node)| Frame(&self.nodes, level, node))
            .collect();
        out.serialize_entry("stack", &stack)?;
        if let Some(base) = self.base_level {
            out.serialize_entry("baseLevel", &base)?;
        }
        out.end()
    }
}
