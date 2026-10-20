//! Pele's `diagrams/requirement/graph.ts`.

use super::{
    db::RequirementModel,
    types::{Relationship, Requirement, RequirementElement},
};

pub struct GraphNode<'a> {
    pub id: &'a str,
    pub requirement: Option<&'a Requirement>,
    pub element: Option<&'a RequirementElement>,
    pub css_styles: &'a [String],
    pub css_classes: String,
}

pub struct GraphEdge<'a> {
    pub id: String,
    pub start: Option<&'a str>,
    pub end: Option<&'a str>,
    pub kind: &'a str,
    pub contains: bool,
}

pub struct RequirementGraph<'a> {
    pub nodes: Vec<GraphNode<'a>>,
    pub edges: Vec<GraphEdge<'a>>,
    pub direction: &'a str,
}

/// The nodes and edges to lay out: requirements first, then elements, as
/// Mermaid orders them.
pub fn build_requirement_graph(model: &RequirementModel) -> RequirementGraph<'_> {
    let mut nodes = Vec::new();
    for requirement in model.requirements.values() {
        nodes.push(GraphNode {
            id: &requirement.name,
            requirement: Some(requirement),
            element: None,
            css_styles: &requirement.css_styles,
            css_classes: requirement.classes.join(" "),
        });
    }
    for element in model.elements.values() {
        nodes.push(GraphNode {
            id: &element.name,
            requirement: None,
            element: Some(element),
            css_styles: &element.css_styles,
            css_classes: element.classes.join(" "),
        });
    }
    let edges = model
        .relations
        .iter()
        .enumerate()
        .map(|(index, relation)| {
            let has =
                |id: &str| model.requirements.contains_key(id) || model.elements.contains_key(id);
            GraphEdge {
                id: format!("{}-{}-{index}", relation.src, relation.dst),
                start: has(&relation.src).then_some(relation.src.as_str()),
                end: has(&relation.dst).then_some(relation.dst.as_str()),
                kind: relation.kind.name(),
                contains: relation.kind == Relationship::Contains,
            }
        })
        .collect();
    RequirementGraph {
        nodes,
        edges,
        direction: &model.direction,
    }
}
