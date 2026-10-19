//! `usecase`: actors, the use cases they take part in and the system
//! boundaries around them. Pele's `diagrams/usecase`.

pub mod db;
pub mod graph;
pub mod json;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod tokens;
pub mod types;

use super::{Diagram, Model};
use crate::mermaid::{Config, Error, Options, ParseLimits, Rendered};

pub use types::UsecaseModel;

pub struct Usecase;

impl Diagram for Usecase {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        Ok(Model::Usecase(Box::new(parser::parse_usecase(
            source, title,
        )?)))
    }

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
        let Model::Usecase(model) = model else {
            unreachable!("usecase renders a usecase model")
        };
        Ok(render::render_usecase(model, config, options))
    }
}
