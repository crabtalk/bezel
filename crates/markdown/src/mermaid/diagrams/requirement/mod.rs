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
use crate::mermaid::{Config, Error, Options, ParseLimits, Rendered};

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
            db.title = Some(title.to_string());
        }
        parser::parse_requirement(source, &mut db)?;
        Ok(Model::Requirement(Box::new(db)))
    }

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
        let Model::Requirement(model) = model else {
            unreachable!("requirement renders a requirement model")
        };
        Ok(render::render_requirement(model, config, options))
    }
}
