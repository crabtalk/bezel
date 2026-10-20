//! `architecture`: services, junctions and groups on a grid, joined by
//! edges between their sides. Pele's `diagrams/architecture`.

pub mod db;
pub mod icons;
pub mod layout;
pub mod parser;
pub mod render;

pub use db::ArchitectureModel;

use super::{Diagram, Model, Render};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};
pub use render::ArchitectureConfig;

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
        db.populate(parser::parse_architecture(source)?, title)?;
        Ok(Model::Architecture(Box::new(db)))
    }
}

impl Render for ArchitectureModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(render::render_architecture(self, &config.architecture, ctx))
    }
}
