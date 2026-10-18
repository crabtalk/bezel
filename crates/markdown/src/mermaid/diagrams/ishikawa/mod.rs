//! `ishikawa`: an effect and the categories of its causes, drawn as a
//! fishbone. Pele's `diagrams/ishikawa`.

pub mod db;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod types;

use super::{Diagram, Model};
use crate::mermaid::{Config, Error, Options, ParseLimits, Rendered};

pub struct Ishikawa;

impl Diagram for Ishikawa {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = db::IshikawaDb::default();
        db.title = title.map(str::to_string);
        parser::parse_ishikawa(source, &mut db)?;
        Ok(Model::Ishikawa(db))
    }

    fn render(
        &self,
        model: &Model,
        _config: &Config,
        options: &Options,
    ) -> Result<Rendered, Error> {
        match model {
            Model::Ishikawa(model) => Ok(render::render_ishikawa(model, options)),
            _ => unreachable!("ishikawa renders an ishikawa model"),
        }
    }
}
