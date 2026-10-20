//! Pele's `diagrams/pie/model.ts`.

use crate::diagrams::common::db::Titles;
use indexmap::IndexMap;
use serde::Serialize;

use super::parser::PieAst;
use crate::{
    detect::Kind,
    error::Error,
    js::{self},
};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PieModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub titles: Titles,
    pub show_data: bool,
    #[serde(serialize_with = "js::json_number_map")]
    pub sections: IndexMap<String, f64>,
}

impl PieModel {
    pub fn new(ast: PieAst, title: Option<&str>) -> Result<Self, Error> {
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
        Ok(Self {
            kind: "pie",
            titles: ast.header.titles(title),
            show_data: ast.show_data,
            sections,
        })
    }
}
