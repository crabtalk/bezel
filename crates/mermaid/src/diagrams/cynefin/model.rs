//! Pele's `diagrams/cynefin/model.ts`.

use crate::diagrams::common::db::Titles;
use indexmap::IndexMap;
use serde::Serialize;

use super::parser::{CynefinAst, DomainBlockAst, TransitionAst};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CynefinItem {
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CynefinDomain {
    pub name: String,
    pub items: Vec<CynefinItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CynefinTransition {
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CynefinModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub titles: Titles,
    /// The domains the source declares. All five are drawn either way.
    pub domains: IndexMap<String, CynefinDomain>,
    pub transitions: Vec<CynefinTransition>,
}

#[derive(Default)]
pub struct CynefinDb {
    pub domains: IndexMap<String, CynefinDomain>,
    pub transitions: Vec<CynefinTransition>,
}

impl CynefinDb {
    /// A later block for a domain replaces an earlier one.
    pub fn set_domains(&mut self, blocks: Vec<DomainBlockAst>) {
        for block in blocks {
            let name = block.domain;
            self.domains.insert(
                name.clone(),
                CynefinDomain {
                    name,
                    items: block
                        .items
                        .into_iter()
                        .map(|label| CynefinItem { label })
                        .collect(),
                },
            );
        }
    }

    pub fn set_transitions(&mut self, transitions: Vec<TransitionAst>) {
        self.transitions = transitions
            .into_iter()
            .filter(|t| t.from != t.to)
            .map(|t| CynefinTransition {
                from: t.from,
                to: t.to,
                label: t.label.filter(|label| !label.is_empty()),
            })
            .collect();
    }
}

impl CynefinModel {
    pub fn new(ast: CynefinAst, title: Option<&str>) -> Self {
        let mut db = CynefinDb::default();
        db.set_domains(ast.domains);
        db.set_transitions(ast.transitions);
        Self {
            kind: "cynefin",
            titles: ast.header.titles(title),
            domains: db.domains,
            transitions: db.transitions,
        }
    }
}
