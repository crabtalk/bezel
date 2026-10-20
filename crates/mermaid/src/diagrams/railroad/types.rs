//! Pele's `diagrams/railroad/types.ts`.
//!
//! A rule's definition is a tree held in one list of nodes, a node naming its
//! parts by their place in it, so that nesting depth is bounded by memory, not
//! by the stack.

use serde::{
    Serialize, Serializer,
    ser::{SerializeMap, SerializeStruct},
};

use crate::js::JsonNumber;

/// A node's place in [`Nodes`].
pub type NodeId = usize;

#[derive(Clone, Debug, PartialEq)]
pub enum RailroadNode {
    Terminal {
        value: String,
    },
    NonTerminal {
        name: String,
    },
    Special {
        text: String,
    },
    Sequence {
        elements: Vec<NodeId>,
    },
    Choice {
        alternatives: Vec<NodeId>,
    },
    Optional {
        element: NodeId,
    },
    /// `max` is infinite when the count has no upper bound.
    Repetition {
        element: NodeId,
        min: f64,
        max: f64,
    },
}

impl RailroadNode {
    /// The node's parts, in order.
    pub fn parts(&self) -> &[NodeId] {
        match self {
            Self::Sequence { elements } => elements,
            Self::Choice { alternatives } => alternatives,
            Self::Optional { element } | Self::Repetition { element, .. } => {
                std::slice::from_ref(element)
            }
            _ => &[],
        }
    }
}

/// Every node of a model's rules.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Nodes(pub Vec<RailroadNode>);

impl Nodes {
    pub fn add(&mut self, node: RailroadNode) -> NodeId {
        self.0.push(node);
        self.0.len() - 1
    }

    pub fn get(&self, id: NodeId) -> &RailroadNode {
        &self.0[id]
    }

    pub fn terminal(&mut self, value: String) -> NodeId {
        self.add(RailroadNode::Terminal { value })
    }

    pub fn nonterminal(&mut self, name: String) -> NodeId {
        self.add(RailroadNode::NonTerminal { name })
    }

    pub fn special(&mut self, text: String) -> NodeId {
        self.add(RailroadNode::Special { text })
    }

    pub fn optional(&mut self, element: NodeId) -> NodeId {
        self.add(RailroadNode::Optional { element })
    }

    pub fn repetition(&mut self, element: NodeId, min: f64, max: f64) -> NodeId {
        self.add(RailroadNode::Repetition { element, min, max })
    }

    /// A sequence of one is that one, in every notation.
    pub fn sequence(&mut self, elements: Vec<NodeId>) -> NodeId {
        if elements.len() == 1 {
            return elements[0];
        }
        self.add(RailroadNode::Sequence { elements })
    }

    /// A choice of one is that one, in every notation.
    pub fn choice(&mut self, alternatives: Vec<NodeId>) -> NodeId {
        if alternatives.len() == 1 {
            return alternatives[0];
        }
        self.add(RailroadNode::Choice { alternatives })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RailroadRule {
    pub name: String,
    pub definition: NodeId,
}

/// Which of Mermaid's four grammars the text was written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Notation {
    Railroad,
    Ebnf,
    Abnf,
    Peg,
}

/// A node of `nodes`, serialized as Pele's nested object.
pub(crate) struct Tree<'a> {
    pub nodes: &'a Nodes,
    pub id: NodeId,
}

impl Serialize for Tree<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let at = |id| Tree {
            nodes: self.nodes,
            id,
        };
        let list = |ids: &[NodeId]| ids.iter().map(|&id| at(id)).collect::<Vec<_>>();
        match self.nodes.get(self.id) {
            RailroadNode::Terminal { value } => {
                let mut out = s.serialize_struct("Node", 2)?;
                out.serialize_field("type", "terminal")?;
                out.serialize_field("value", value)?;
                out.end()
            }
            RailroadNode::NonTerminal { name } => {
                let mut out = s.serialize_struct("Node", 2)?;
                out.serialize_field("type", "nonterminal")?;
                out.serialize_field("name", name)?;
                out.end()
            }
            RailroadNode::Special { text } => {
                let mut out = s.serialize_struct("Node", 2)?;
                out.serialize_field("type", "special")?;
                out.serialize_field("text", text)?;
                out.end()
            }
            RailroadNode::Sequence { elements } => {
                let mut out = s.serialize_struct("Node", 2)?;
                out.serialize_field("type", "sequence")?;
                out.serialize_field("elements", &list(elements))?;
                out.end()
            }
            RailroadNode::Choice { alternatives } => {
                let mut out = s.serialize_struct("Node", 2)?;
                out.serialize_field("type", "choice")?;
                out.serialize_field("alternatives", &list(alternatives))?;
                out.end()
            }
            RailroadNode::Optional { element } => {
                let mut out = s.serialize_struct("Node", 2)?;
                out.serialize_field("type", "optional")?;
                out.serialize_field("element", &at(*element))?;
                out.end()
            }
            RailroadNode::Repetition { element, min, max } => {
                let mut out = s.serialize_struct("Node", 4)?;
                out.serialize_field("type", "repetition")?;
                out.serialize_field("element", &at(*element))?;
                out.serialize_field("min", &JsonNumber(*min))?;
                out.serialize_field("max", &JsonNumber(*max))?;
                out.end()
            }
        }
    }
}

/// A rule of `nodes`, serialized as Pele's `{ name, definition }`.
pub(crate) struct Rule<'a> {
    pub nodes: &'a Nodes,
    pub rule: &'a RailroadRule,
}

impl Serialize for Rule<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out = s.serialize_map(Some(2))?;
        out.serialize_entry("name", &self.rule.name)?;
        out.serialize_entry(
            "definition",
            &Tree {
                nodes: self.nodes,
                id: self.rule.definition,
            },
        )?;
        out.end()
    }
}
