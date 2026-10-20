//! `treemap`: nested values as nested rectangles. Pele's
//! `diagrams/treemap`.

pub mod format;
pub mod layout;
pub mod model;
pub mod parser;
pub mod render;

pub use model::TreemapModel;

use super::{Diagram, Model, Render};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};
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

impl Render for TreemapModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(render::render_treemap(self, &config.treemap, ctx))
    }
}
