//! Pele's `diagrams/class/graph.ts`.

use std::collections::HashMap;

use super::{
    db::ClassModel,
    types::{ClassNode, LineType, RelationEnd, RelationType},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeShape {
    Rect,
    ClassBox,
    Note,
    Interface,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Solid,
    Dashed,
    Dotted,
}

pub struct GraphNode<'m> {
    pub id: &'m str,
    pub label: &'m str,
    pub is_group: bool,
    pub shape: NodeShape,
    pub parent_id: Option<&'m str>,
    /// Index of the namespace node this one is drawn in.
    pub parent: Option<usize>,
    pub class_node: Option<&'m ClassNode>,
    /// Position among the classes, in declaration order. Namespaces and notes
    /// have none.
    pub color_index: Option<usize>,
}

pub struct GraphEdge<'m> {
    pub id: String,
    pub start: &'m str,
    pub end: &'m str,
    /// Indexes into the node list. Ids alone are ambiguous: a class may be
    /// named `note0` or `interface0`.
    pub source: Option<usize>,
    pub target: Option<usize>,
    pub label: Option<&'m str>,
    pub arrow_type_start: Option<RelationType>,
    pub arrow_type_end: Option<RelationType>,
    pub start_label_right: &'m str,
    pub end_label_left: &'m str,
    pub pattern: Pattern,
}

pub struct ClassGraph<'m> {
    pub nodes: Vec<GraphNode<'m>>,
    pub edges: Vec<GraphEdge<'m>>,
}

fn end_title(text: &str) -> &str {
    if text == "none" { "" } else { text }
}

/// The nearest namespace around `id` that the source declares, with
/// `hierarchical` off; `declared` remembers the answer for each namespace
/// passed on the way.
fn declared_ancestor<'m>(
    model: &'m ClassModel,
    hierarchical: bool,
    declared: &mut HashMap<&'m str, Option<&'m str>>,
    id: Option<&'m str>,
) -> Option<&'m str> {
    let id = id?;
    if hierarchical {
        return Some(id);
    }
    let mut chain: Vec<&str> = Vec::new();
    let mut current = Some(id);
    let mut found = None;
    while let Some(at) = current.filter(|at| !at.is_empty()) {
        if let Some(&known) = declared.get(at) {
            found = known;
            break;
        }
        let Some((key, ns)) = model.namespaces.get_key_value(at) else {
            break;
        };
        if ns.explicit {
            found = Some(key.as_str());
            break;
        }
        chain.push(key.as_str());
        current = ns.parent.as_deref();
    }
    for visited in chain {
        declared.insert(visited, found);
    }
    found
}

impl ClassModel {
    /// The nodes and edges that are drawn. With `hierarchical` off, only
    /// namespaces the source declares are kept and each is labeled with its full
    /// name.
    pub fn graph(&self, hierarchical: bool) -> ClassGraph<'_> {
        let model = self;
        let mut nodes: Vec<GraphNode> = Vec::new();
        let mut edges: Vec<GraphEdge> = Vec::new();
        let mut namespace_at: HashMap<&str, usize> = HashMap::new();
        let mut class_at: HashMap<&str, usize> = HashMap::new();
        let mut interface_at: HashMap<&str, usize> = HashMap::new();

        let mut declared: HashMap<&str, Option<&str>> = HashMap::new();

        for ns in model.namespaces.values() {
            if !hierarchical && !ns.explicit {
                continue;
            }
            namespace_at.insert(&ns.id, nodes.len());
            nodes.push(GraphNode {
                id: &ns.id,
                label: if hierarchical { &ns.label } else { &ns.id },
                is_group: true,
                shape: NodeShape::Rect,
                parent_id: if hierarchical {
                    ns.parent.as_deref()
                } else {
                    None
                },
                parent: None,
                class_node: None,
                color_index: None,
            });
        }
        let group = |namespace_at: &HashMap<&str, usize>, id: Option<&str>| {
            id.and_then(|id| namespace_at.get(id)).copied()
        };
        for node in nodes.iter_mut() {
            node.parent = group(&namespace_at, node.parent_id);
        }

        for (color_index, class_node) in model.classes.values().enumerate() {
            let parent_id = declared_ancestor(
                model,
                hierarchical,
                &mut declared,
                class_node.parent.as_deref(),
            );
            class_at.insert(&class_node.id, nodes.len());
            nodes.push(GraphNode {
                id: &class_node.id,
                label: &class_node.label,
                is_group: false,
                shape: NodeShape::ClassBox,
                parent_id,
                parent: group(&namespace_at, parent_id),
                class_node: Some(class_node),
                color_index: Some(color_index),
            });
        }

        for note in model.notes.values() {
            let parent_id =
                declared_ancestor(model, hierarchical, &mut declared, note.parent.as_deref());
            let target = note
                .class
                .as_deref()
                .and_then(|class| class_at.get(class).copied());
            if let Some(target) = target {
                edges.push(GraphEdge {
                    id: format!("edgeNote{}", note.index),
                    start: &note.id,
                    end: nodes[target].id,
                    source: Some(nodes.len()),
                    target: Some(target),
                    label: None,
                    arrow_type_start: None,
                    arrow_type_end: None,
                    start_label_right: "",
                    end_label_left: "",
                    pattern: Pattern::Dotted,
                });
            }
            nodes.push(GraphNode {
                id: &note.id,
                label: &note.text,
                is_group: false,
                shape: NodeShape::Note,
                parent_id,
                parent: group(&namespace_at, parent_id),
                class_node: None,
                color_index: None,
            });
        }

        // Mermaid leaves every lollipop interface outside the namespaces. Here it
        // stays beside its class.
        for item in &model.interfaces {
            let owner = class_at.get(item.class_id.as_str()).copied();
            interface_at.insert(&item.id, nodes.len());
            let parent = owner.and_then(|owner| nodes[owner].parent);
            nodes.push(GraphNode {
                id: &item.id,
                label: &item.label,
                is_group: false,
                shape: NodeShape::Interface,
                parent_id: None,
                parent,
                class_node: None,
                color_index: None,
            });
        }

        for (count, relation) in model.relations.iter().enumerate() {
            let (type1, type2) = (relation.relation.type1, relation.relation.type2);
            // add_relation swaps in an interface for the lollipop end of a relation
            // with no other marker.
            let start_is_interface =
                type1 == RelationEnd::Type(RelationType::Lollipop) && type2 == RelationEnd::None;
            let end_is_interface = !start_is_interface
                && type2 == RelationEnd::Type(RelationType::Lollipop)
                && type1 == RelationEnd::None;
            let start_at = if start_is_interface {
                &interface_at
            } else {
                &class_at
            };
            let end_at = if end_is_interface {
                &interface_at
            } else {
                &class_at
            };
            edges.push(GraphEdge {
                id: format!("id_{}_{}_{}", relation.id1, relation.id2, count + 1),
                start: &relation.id1,
                end: &relation.id2,
                source: start_at.get(relation.id1.as_str()).copied(),
                target: end_at.get(relation.id2.as_str()).copied(),
                label: relation.title.as_deref(),
                arrow_type_start: type1.marker(),
                arrow_type_end: type2.marker(),
                start_label_right: end_title(&relation.relation_title1),
                end_label_left: end_title(&relation.relation_title2),
                pattern: if relation.relation.line_type == LineType::Dotted {
                    Pattern::Dashed
                } else {
                    Pattern::Solid
                },
            });
        }

        ClassGraph { nodes, edges }
    }
}
