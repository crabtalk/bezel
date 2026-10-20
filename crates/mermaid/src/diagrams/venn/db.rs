//! Pele's `diagrams/venn/db.ts`.

use crate::util::chars::unquote;
use indexmap::{IndexMap, IndexSet};
use serde::Serialize;

use super::{
    lexer::IndentHost,
    types::{VennStyle, VennSubset, VennText},
};
use crate::{detect::Kind, error::Error, js};

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VennDb {
    #[serde(rename = "type")]
    pub kind: VennType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub subsets: Vec<VennSubset>,
    pub text_nodes: Vec<VennText>,
    pub styles: Vec<VennStyle>,
    known: IndexSet<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current: Option<Vec<String>>,
    /// Whether an indented `text` line belongs to the set or union before
    /// it. The lexer reads this.
    indent: bool,
}

/// The model's `type`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub enum VennType {
    #[default]
    #[serde(rename = "venn")]
    Venn,
}

/// Mermaid takes one pair of quotes off an id, label or style value.
pub fn normalize_text(text: &str) -> String {
    unquote(js::trim(text)).to_string()
}

fn normalize_ids(ids: &[String]) -> Vec<String> {
    let mut sets: Vec<String> = ids.iter().map(|id| normalize_text(id)).collect();
    js::sort_strings(&mut sets);
    sets
}

impl VennDb {
    /// A set without a size gets 10, and an overlap of n sets 10 / n².
    pub fn add_subset_data(&mut self, ids: &[String], label: Option<&str>, size: Option<f64>) {
        let sets = normalize_ids(ids);
        self.current = Some(sets.clone());
        if sets.len() == 1 {
            self.known.insert(sets[0].clone());
        }
        let n = ids.len() as f64;
        self.subsets.push(VennSubset {
            sets,
            size: size.unwrap_or(10.0 / (n * n)),
            label: label.filter(|label| !label.is_empty()).map(normalize_text),
        });
    }

    pub fn add_text_data(&mut self, ids: &[String], id: &str, label: Option<&str>) {
        self.text_nodes.push(VennText {
            sets: normalize_ids(ids),
            id: normalize_text(id),
            label: label.filter(|label| !label.is_empty()).map(normalize_text),
        });
    }

    pub fn add_style_data(&mut self, ids: &[String], data: Vec<(String, String)>) {
        let mut styles = IndexMap::new();
        for (key, value) in data {
            let value = if value.is_empty() {
                value
            } else {
                normalize_text(&value)
            };
            styles.insert(key, value);
        }
        self.styles.push(VennStyle {
            targets: normalize_ids(ids),
            styles,
        });
    }

    pub fn validate_union_identifiers(&self, ids: &[String]) -> Result<(), Error> {
        let unknown: Vec<String> = ids
            .iter()
            .map(|id| normalize_text(id))
            .filter(|id| !self.known.contains(id))
            .collect();
        if !unknown.is_empty() {
            return Err(
                Error::semantic(format!("unknown set identifier: {}", unknown.join(", ")))
                    .of(Kind::Venn),
            );
        }
        Ok(())
    }

    pub fn current_sets(&self) -> Option<&[String]> {
        self.current.as_deref()
    }
}

impl IndentHost for VennDb {
    fn indent_mode(&self) -> bool {
        self.indent
    }

    fn set_indent_mode(&mut self, enabled: bool) {
        self.indent = enabled;
    }
}
