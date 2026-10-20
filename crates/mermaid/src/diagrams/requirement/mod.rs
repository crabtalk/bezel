//! `requirementDiagram`: requirements, the elements that meet them, and how
//! they relate. Pele's `diagrams/requirement`.

pub mod db;
pub mod graph;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod tokens;
pub mod types;

pub use db::RequirementModel;

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits};

pub struct Requirement;

impl Diagram for Requirement {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = RequirementModel::default();
        if let Some(title) = title.filter(|title| !title.is_empty()) {
            db.titles.set_title(title);
        }
        parser::parse_requirement(source, &mut db)?;
        Ok(Model::Requirement(Box::new(db)))
    }
}
