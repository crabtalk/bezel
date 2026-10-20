//! Pele's `diagrams/requirement/graph.ts`.

use super::{
    db::RequirementModel,
    types::{Relationship, Requirement, RequirementElement},
};

/// What a node draws.
#[derive(Clone, Copy)]
pub enum NodeItem<'a> {
    Requirement(&'a Requirement),
    Element(&'a RequirementElement),
}

pub struct GraphNode<'a> {
    pub id: &'a str,
    pub item: NodeItem<'a>,
    pub css_styles: &'a [String],
    pub css_classes: String,
}

pub struct GraphEdge<'a> {
    pub id: String,
    pub start: Option<&'a str>,
    pub end: Option<&'a str>,
    pub kind: Relationship,
}

pub struct RequirementGraph<'a> {
    pub nodes: Vec<GraphNode<'a>>,
    pub edges: Vec<GraphEdge<'a>>,
}

impl RequirementModel {
    /// The nodes and edges to lay out: requirements first, then elements, as
    /// Mermaid orders them.
    pub fn graph(&self) -> RequirementGraph<'_> {
        let mut nodes = Vec::new();
        for requirement in self.requirements.values() {
            nodes.push(GraphNode {
                id: &requirement.name,
                item: NodeItem::Requirement(requirement),
                css_styles: &requirement.css_styles,
                css_classes: requirement.classes.join(" "),
            });
        }
        for element in self.elements.values() {
            nodes.push(GraphNode {
                id: &element.name,
                item: NodeItem::Element(element),
                css_styles: &element.css_styles,
                css_classes: element.classes.join(" "),
            });
        }
        let edges = self
            .relations
            .iter()
            .enumerate()
            .map(|(index, relation)| {
                let has =
                    |id: &str| self.requirements.contains_key(id) || self.elements.contains_key(id);
                GraphEdge {
                    id: format!("{}-{}-{index}", relation.src, relation.dst),
                    start: has(&relation.src).then_some(relation.src.as_str()),
                    end: has(&relation.dst).then_some(relation.dst.as_str()),
                    kind: relation.kind,
                }
            })
            .collect();
        RequirementGraph { nodes, edges }
    }
}
