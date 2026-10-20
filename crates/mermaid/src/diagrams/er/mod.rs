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

use super::{Diagram, Model, Render};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};

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
            db.titles.set_title(title);
        }
        parser::parse_er(source, &mut db)?;
        Ok(Model::Er(Box::new(db)))
    }
}

impl Render for ErModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(render::render_er(self, config, ctx))
    }
}
