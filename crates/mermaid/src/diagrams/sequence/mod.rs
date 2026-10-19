//! `sequenceDiagram`: participants and the messages between them, top to
//! bottom. Pele's `diagrams/sequence`.

pub mod actors;
pub mod colors;
pub mod db;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod tokens;
pub mod types;

pub use db::SequenceModel;

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits, Rendered, config::Value};
use crate::{Ctx, Host};

pub struct Sequence;

impl Diagram for Sequence {
    fn parse(
        &self,
        source: &str,
        config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let wrap = match config.get("wrap") {
            Some(Value::Bool(wrap)) => Some(*wrap),
            _ => None,
        };
        let sequence_wrap = matches!(
            config.section("sequence").and_then(|seq| seq.get("wrap")),
            Some(Value::Bool(true))
        );
        let mut db = SequenceModel::new(wrap, sequence_wrap);
        if let Some(title) = title.filter(|title| !title.is_empty()) {
            db.set_diagram_title(title);
        }
        parser::parse_sequence(source, &mut db)?;
        Ok(Model::Sequence(Box::new(db)))
    }
}

pub fn render(model: &Model, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
    let Model::Sequence(model) = model else {
        unreachable!("sequenceDiagram renders a sequence model")
    };
    Ok(render::render_sequence(model, config, ctx))
}
