//! Pele's `diagrams/sankey/db.ts`.

use indexmap::IndexMap;
use serde::Serialize;

use crate::{detect::Kind, diagrams::common::db::Titles, js};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SankeyNode {
    pub id: String,
    pub index: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SankeyLink {
    pub source: SankeyNode,
    pub target: SankeyNode,
    #[serde(serialize_with = "js::json_number")]
    pub value: f64,
}

/// Pele's `SankeyDb`, serialized with its lookup as Pele's is.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SankeyModel {
    #[serde(rename = "type")]
    pub kind: Kind,
    pub nodes: Vec<SankeyNode>,
    pub links: Vec<SankeyLink>,
    #[serde(flatten)]
    pub titles: Titles,
    lookup: IndexMap<String, SankeyNode>,
}

impl Default for SankeyModel {
    fn default() -> Self {
        Self {
            kind: Kind::Sankey,
            nodes: Vec::new(),
            links: Vec::new(),
            titles: Titles::default(),
            lookup: IndexMap::new(),
        }
    }
}

impl SankeyModel {
    pub fn find_or_create_node(&mut self, id: &str) -> SankeyNode {
        if let Some(node) = self.lookup.get(id) {
            return node.clone();
        }
        let node = SankeyNode {
            id: id.to_string(),
            index: self.nodes.len(),
        };
        self.lookup.insert(id.to_string(), node.clone());
        self.nodes.push(node.clone());
        node
    }

    pub fn add_link(&mut self, source: SankeyNode, target: SankeyNode, value: f64) {
        self.links.push(SankeyLink {
            source,
            target,
            value,
        });
    }
}
