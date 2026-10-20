//! Pele's `diagrams/ishikawa/types.ts`.
//!
//! The outline is held in one list of nodes, a node naming its children by
//! their place in it, so that nesting depth is bounded by memory, not by the
//! stack.

use serde::{Serialize, Serializer, ser::SerializeMap};

/// A node's place in [`Nodes`].
pub type NodeId = usize;

/// The root is the effect; its children are the categories of causes, theirs
/// the causes, and so on.
#[derive(Clone, Debug, PartialEq)]
pub struct IshikawaNode {
    pub text: String,
    pub children: Vec<NodeId>,
}

/// Every node of an outline.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Nodes(pub Vec<IshikawaNode>);

impl Nodes {
    pub fn add(&mut self, text: String) -> NodeId {
        self.0.push(IshikawaNode {
            text,
            children: Vec::new(),
        });
        self.0.len() - 1
    }

    pub fn get(&self, id: NodeId) -> &IshikawaNode {
        &self.0[id]
    }
}

/// A node of `nodes`, serialized as Pele's nested `{ text, children }`.
pub(crate) struct Tree<'a> {
    pub nodes: &'a Nodes,
    pub id: NodeId,
}

impl Serialize for Tree<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let node = self.nodes.get(self.id);
        let children: Vec<Tree> = node
            .children
            .iter()
            .map(|&id| Tree {
                nodes: self.nodes,
                id,
            })
            .collect();
        let mut out = s.serialize_map(Some(2))?;
        out.serialize_entry("text", &node.text)?;
        out.serialize_entry("children", &children)?;
        out.end()
    }
}
