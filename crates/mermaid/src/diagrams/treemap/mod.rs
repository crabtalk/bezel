//! `treemap`: nested values as nested rectangles. Pele's
//! `diagrams/treemap`.

pub mod format;
pub mod layout;
pub mod model;
pub mod parser;
pub mod render;

pub use model::TreemapModel;

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits};
pub use render::TreemapConfig;

pub struct Treemap;

impl Diagram for Treemap {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        Ok(Model::Treemap(TreemapModel::new(
            parser::parse_treemap(source)?,
            title,
        )))
    }
}
