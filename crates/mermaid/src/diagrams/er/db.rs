//! Pele's `diagrams/er/db.ts`.

use crate::diagrams::common::db::Titles;
use crate::diagrams::common::style_class::StyleClass;
use crate::geom::Direction;
use indexmap::{IndexMap, IndexSet};
use serde::{Serialize, Serializer};

use super::types::{
    DocItem, ErAttribute, ErEntity, ErRelationship, ErSubgraph, LabelType, ParsedAttribute, RelSpec,
};
use crate::js;

/// Pele's `ErDb`, serialized with its lookups as Pele's is.
#[derive(Clone, Debug, PartialEq)]
pub struct ErModel {
    pub direction: Direction,
    pub entities: IndexMap<String, ErEntity>,
    pub relationships: Vec<ErRelationship>,
    pub classes: IndexMap<String, StyleClass>,
    pub subgraphs: Vec<ErSubgraph>,
    pub titles: Titles,
    /// The subgraph of each id: an index into `subgraphs`, the last one
    /// declared with the id.
    subgraph_lookup: IndexMap<String, usize>,
    claimed: IndexSet<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Json<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    direction: Direction,
    entities: &'a IndexMap<String, ErEntity>,
    relationships: &'a [ErRelationship],
    classes: &'a IndexMap<String, StyleClass>,
    subgraphs: &'a [ErSubgraph],
    #[serde(flatten)]
    titles: &'a Titles,
    subgraph_lookup: IndexMap<&'a str, &'a ErSubgraph>,
    claimed: &'a IndexSet<String>,
}

impl Serialize for ErModel {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        Json {
            kind: "er",
            direction: self.direction,
            entities: &self.entities,
            relationships: &self.relationships,
            classes: &self.classes,
            subgraphs: &self.subgraphs,
            titles: &self.titles,
            subgraph_lookup: self
                .subgraph_lookup
                .iter()
                .map(|(id, &at)| (id.as_str(), &self.subgraphs[at]))
                .collect(),
            claimed: &self.claimed,
        }
        .serialize(s)
    }
}

impl Default for ErModel {
    fn default() -> Self {
        Self {
            direction: Direction::TopToBottom,
            entities: IndexMap::new(),
            relationships: Vec::new(),
            classes: IndexMap::new(),
            subgraphs: Vec::new(),
            titles: Titles::default(),
            subgraph_lookup: IndexMap::new(),
            claimed: IndexSet::new(),
        }
    }
}

impl ErModel {
    pub fn add_entity(&mut self, name: &str, alias: &str) -> &mut ErEntity {
        let size = self.entities.len();
        let entity = self
            .entities
            .entry(name.to_string())
            .or_insert_with(|| ErEntity {
                id: format!("entity-{name}-{size}"),
                label: name.to_string(),
                alias: alias.to_string(),
                attributes: Vec::new(),
                css_classes: "default".into(),
                css_styles: Vec::new(),
            });
        if entity.alias.is_empty() && !alias.is_empty() {
            entity.alias = alias.to_string();
        }
        entity
    }

    /// The grammar builds the list back to front, so the last attribute
    /// written comes first.
    pub fn add_attributes(&mut self, name: &str, attributes: Vec<ParsedAttribute>) {
        let entity = self.add_entity(name, "");
        for attribute in attributes.into_iter().rev() {
            entity.attributes.push(ErAttribute {
                kind: attribute.kind,
                name: attribute.name,
                keys: attribute.keys.unwrap_or_default(),
                comment: attribute.comment.unwrap_or_default(),
            });
        }
    }

    pub fn add_relationship(&mut self, ent_a: &str, role_a: &str, ent_b: &str, rel_spec: RelSpec) {
        let entity_a = if self.subgraph_lookup.contains_key(ent_a) {
            ent_a.to_string()
        } else {
            self.add_entity(ent_a, "").id.clone()
        };
        let entity_b = if self.subgraph_lookup.contains_key(ent_b) {
            ent_b.to_string()
        } else {
            self.add_entity(ent_b, "").id.clone()
        };
        self.relationships.push(ErRelationship {
            entity_a,
            role_a: role_a.to_string(),
            entity_b,
            rel_spec,
        });
    }

    pub fn add_css_styles(&mut self, ids: &[&str], styles: &[String]) {
        for id in ids {
            if let Some(entity) = self.entities.get_mut(*id) {
                entity.css_styles.extend(styles.iter().cloned());
            }
            if let Some(&at) = self.subgraph_lookup.get(*id) {
                self.subgraphs[at].css_styles.extend(styles.iter().cloned());
            }
        }
    }

    pub fn add_class(&mut self, ids: &[&str], styles: &[String]) {
        for id in ids {
            let def = self
                .classes
                .entry(id.to_string())
                .or_insert_with(|| StyleClass::new(id));
            for style in styles {
                def.push(style);
            }
        }
    }

    pub fn set_class(&mut self, ids: &[&str], class_names: &[&str]) {
        for id in ids {
            if let Some(entity) = self.entities.get_mut(*id) {
                for name in class_names {
                    entity.css_classes.push(' ');
                    entity.css_classes.push_str(name);
                }
            }
            if let Some(&at) = self.subgraph_lookup.get(*id) {
                self.subgraphs[at]
                    .classes
                    .extend(class_names.iter().map(|name| name.to_string()));
            }
        }
    }

    pub fn add_sub_graph(&mut self, id_text: &str, list: &[DocItem], title_text: &str) -> String {
        let id = js::trim(id_text).to_string();
        let mut seen: IndexSet<&str> = IndexSet::new();
        let mut nodes: Vec<String> = Vec::new();
        let mut dir = None;
        for item in list {
            let item = match item {
                DocItem::Dir(value) => {
                    dir = Some(*value);
                    continue;
                }
                DocItem::Name(item) => item,
            };
            let trimmed = js::trim(item);
            if trimmed.is_empty() || seen.contains(trimmed) {
                continue;
            }
            seen.insert(trimmed);
            // A node belongs to the first subgraph that lists it.
            if !self.claimed.contains(item.as_str()) {
                nodes.push(item.clone());
            }
        }
        for node in &nodes {
            self.claimed.insert(node.clone());
        }
        self.subgraphs.push(ErSubgraph {
            id: id.clone(),
            nodes,
            title: js::trim(title_text).to_string(),
            classes: Vec::new(),
            css_styles: Vec::new(),
            dir,
            label_type: LabelType::Markdown,
        });
        self.subgraph_lookup
            .insert(id.clone(), self.subgraphs.len() - 1);
        id
    }
}
