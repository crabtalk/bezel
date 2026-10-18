//! Pele's `diagrams/pie/model.ts`.

use indexmap::IndexMap;
use serde::Serialize;

use super::parser::PieAst;
use crate::mermaid::{
    detect::Kind,
    error::Error,
    js::{self, Lazy},
};

static RE_LEAD: Lazy = Lazy::new(r"^\s+", "g");
static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PieModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_descr: Option<String>,
    pub show_data: bool,
    #[serde(serialize_with = "js::json_number_map")]
    pub sections: IndexMap<String, f64>,
}

pub fn build_pie(ast: PieAst, title: Option<&str>) -> Result<PieModel, Error> {
    let mut sections = IndexMap::new();
    for section in ast.sections {
        if section.value < 0.0 {
            return Err(Error::semantic(format!(
                "\"{}\" has invalid value: {}. Negative values are not allowed in pie charts. All slice values must be >= 0.",
                section.label,
                js::num_str(section.value)
            ))
            .of(Kind::Pie));
        }
        sections.entry(section.label).or_insert(section.value);
    }
    Ok(PieModel {
        kind: "pie",
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
        show_data: ast.show_data,
        sections,
    })
}
