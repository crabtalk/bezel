//! `c4`: Mermaid's C4 context, container, component, dynamic and deployment
//! diagrams. Pele's `diagrams/c4`.

pub mod db;
pub mod layout;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod route;
pub mod tokens;
pub mod types;

pub use db::C4Model;

use super::{Diagram, Model};
use crate::mermaid::{Config, Error, Options, ParseLimits, Rendered};

pub struct C4;

impl Diagram for C4 {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = C4Model::default();
        if let Some(title) = title.filter(|title| !title.is_empty()) {
            db.title = Some(title.to_string());
        }
        parser::parse_c4(source, &mut db)?;
        Ok(Model::C4(db))
    }

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
        let Model::C4(model) = model else {
            unreachable!("c4 renders a c4 model")
        };
        render::render_c4(model, config, options)
    }
}
