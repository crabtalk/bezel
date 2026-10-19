//! `erDiagram`: entities, their attributes and the relationships between
//! them. Pele's `diagrams/er`.

pub mod db;
pub mod graph;
pub mod lexer;
pub mod markers;
pub mod parser;
pub mod render;
pub mod tokens;
pub mod types;

pub use db::ErModel;

use super::{Diagram, Model};
use crate::mermaid::{Config, Error, Options, ParseLimits, Rendered};

pub struct Er;

impl Diagram for Er {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = ErModel::default();
        if let Some(title) = title.filter(|title| !title.is_empty()) {
            db.title = Some(title.to_string());
        }
        parser::parse_er(source, &mut db)?;
        Ok(Model::Er(Box::new(db)))
    }

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
        let Model::Er(model) = model else {
            unreachable!("er renders an er model")
        };
        Ok(render::render_er(model, config, options))
    }
}
