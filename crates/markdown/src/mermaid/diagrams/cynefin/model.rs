//! Pele's `diagrams/cynefin/model.ts`.

use indexmap::IndexMap;
use serde::Serialize;

use super::parser::{CynefinAst, DomainBlockAst, TransitionAst};
use crate::mermaid::js::Lazy;

static RE_LEAD: Lazy = Lazy::new(r"^\s+", "g");
static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_descr: Option<String>,
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

pub fn build_cynefin(ast: CynefinAst, title: Option<&str>) -> CynefinModel {
    let mut db = CynefinDb::default();
    db.set_domains(ast.domains);
    db.set_transitions(ast.transitions);
    CynefinModel {
        kind: "cynefin",
        title: ast
            .title
            .filter(|title| !title.is_empty())
            .or_else(|| title.map(str::to_string)),
        acc_title: ast
            .acc_title
            .filter(|text| !text.is_empty())
            .map(|text| RE_LEAD.replace(&text, |_| String::new())),
        acc_descr: ast
            .acc_descr
            .filter(|text| !text.is_empty())
            .map(|text| RE_INDENT.replace(&text, |_| "\n".into())),
        domains: db.domains,
        transitions: db.transitions,
    }
}
