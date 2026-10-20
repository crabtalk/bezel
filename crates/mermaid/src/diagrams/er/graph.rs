//! Pele's `diagrams/er/graph.ts`.

use std::collections::{HashMap, HashSet};

use super::{
    db::ErModel,
    types::{Cardinality, ErAttribute, Identification},
};
use crate::geom::Direction;

pub struct ErGraphNode<'a> {
    pub id: &'a str,
    pub label: &'a str,
    pub alias: &'a str,
    pub attributes: &'a [ErAttribute],
    pub is_group: bool,
    pub parent_id: Option<&'a str>,
    pub css_styles: &'a [String],
    pub css_compiled_styles: Vec<String>,
    pub css_classes: String,
    pub dir: Option<Direction>,
}

pub struct ErGraphEdge<'a> {
    pub id: String,
    pub start: &'a str,
    pub end: &'a str,
    pub label: &'a str,
    pub arrow_type_start: Cardinality,
    pub arrow_type_end: Cardinality,
    pub dashed: bool,
}

pub struct ErGraph<'a> {
    pub nodes: Vec<ErGraphNode<'a>>,
    pub edges: Vec<ErGraphEdge<'a>>,
}

impl ErModel {
    /// The diagram resolved into what is drawn: subgraphs as groups,
    /// entities as nodes with their class styles, and relationships as edges
    /// with a marker type at each end.
    pub fn graph(&self) -> ErGraph<'_> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let subgraphs = &self.subgraphs;

        let compiled = |class_names: &mut dyn Iterator<Item = &str>| -> Vec<String> {
            let mut out = Vec::new();
            for name in class_names {
                let Some(def) = self.classes.get(name) else {
                    continue;
                };
                for style in &def.styles {
                    out.push(crate::js::trim(style).to_string());
                }
                for style in &def.text_styles {
                    out.push(crate::js::trim(style).to_string());
                }
            }
            out
        };

        let mut parent_of: HashMap<&str, &str> = HashMap::new();
        let mut subgraph_ids: HashSet<&str> = HashSet::new();
        for subgraph in subgraphs.iter().rev() {
            subgraph_ids.insert(&subgraph.id);
            for id in &subgraph.nodes {
                parent_of.insert(id, &subgraph.id);
            }
        }

        for subgraph in subgraphs.iter().rev() {
            nodes.push(ErGraphNode {
                id: &subgraph.id,
                label: &subgraph.title,
                alias: "",
                attributes: &[],
                is_group: true,
                parent_id: parent_of.get(subgraph.id.as_str()).copied(),
                css_styles: &subgraph.css_styles,
                css_compiled_styles: compiled(&mut subgraph.classes.iter().map(String::as_str)),
                css_classes: subgraph.classes.join(" "),
                dir: subgraph.dir,
            });
        }

        // A name that is also a subgraph id stands for the subgraph. Mermaid
        // drops such an entity and leaves relationships written before the
        // subgraph pointing at nothing; here they follow it.
        let mut moved: HashMap<&str, &str> = HashMap::new();
        for (name, entity) in &self.entities {
            if subgraph_ids.contains(name.as_str()) {
                moved.insert(&entity.id, name);
                continue;
            }
            nodes.push(ErGraphNode {
                id: &entity.id,
                label: &entity.label,
                alias: &entity.alias,
                attributes: &entity.attributes,
                is_group: false,
                parent_id: parent_of.get(name.as_str()).copied(),
                css_styles: &entity.css_styles,
                css_compiled_styles: compiled(&mut entity.css_classes.split(' ')),
                css_classes: entity.css_classes.clone(),
                dir: None,
            });
        }

        for (index, relationship) in self.relationships.iter().enumerate() {
            edges.push(ErGraphEdge {
                id: format!(
                    "id_{}_{}_{index}",
                    relationship.entity_a, relationship.entity_b
                ),
                start: moved
                    .get(relationship.entity_a.as_str())
                    .copied()
                    .unwrap_or(&relationship.entity_a),
                end: moved
                    .get(relationship.entity_b.as_str())
                    .copied()
                    .unwrap_or(&relationship.entity_b),
                label: &relationship.role_a,
                arrow_type_start: relationship.rel_spec.card_b,
                arrow_type_end: relationship.rel_spec.card_a,
                dashed: relationship.rel_spec.rel_type != Identification::Identifying,
            });
        }

        ErGraph { nodes, edges }
    }
}
