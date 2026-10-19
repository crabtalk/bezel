//! `architecture`: services, junctions and groups on a grid, joined by
//! edges between their sides. Pele's `diagrams/architecture`.

pub mod db;
pub mod icons;
pub mod layout;
pub mod parser;
pub mod render;

pub use db::ArchitectureModel;

use super::{Diagram, Model};
use crate::{Config, Error, Options, ParseLimits, Rendered};

pub struct Architecture;

impl Diagram for Architecture {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = ArchitectureModel::new();
        if let Some(title) = title.filter(|title| !title.is_empty()) {
            db.title = Some(title.to_string());
        }
        db::populate(parser::parse_architecture(source)?, &mut db)?;
        Ok(Model::Architecture(Box::new(db)))
    }

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
        let Model::Architecture(model) = model else {
            unreachable!("architecture renders an architecture model")
        };
        Ok(render::render_architecture(model, config, options))
    }
}
