//! `venn`: sets drawn as circles, overlapping as their unions are stated.
//! Pele's `diagrams/venn`.

pub mod db;
pub mod layout;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod types;

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};

pub struct Venn;

impl Diagram for Venn {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = db::VennDb::default();
        if let Some(title) = title.filter(|title| !title.is_empty()) {
            db.title = Some(title.to_string());
        }
        parser::parse_venn(source, &mut db)?;
        Ok(Model::Venn(db))
    }
}

pub fn render(model: &Model, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
    match model {
        Model::Venn(model) => Ok(render::render_venn(model, config, ctx)),
        _ => unreachable!("venn renders a venn model"),
    }
}
