//! `architecture`: services, junctions and groups on a grid, joined by
//! edges between their sides. Pele's `diagrams/architecture`.

pub mod db;
pub mod icons;
pub mod layout;
pub mod parser;
pub mod render;

pub use db::ArchitectureModel;

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits};
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
