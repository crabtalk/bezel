//! `pie`: slices of a whole. Pele's `diagrams/pie`.

pub mod model;
pub mod parser;
pub mod render;

pub use model::PieModel;

use super::{Diagram, Model, Render};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};
pub use render::PieConfig;

pub struct Pie;

impl Diagram for Pie {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        Ok(Model::Pie(PieModel::new(
            parser::parse_pie(source)?,
            title,
        )?))
    }
}

impl Render for PieModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(render::render_pie(self, &config.pie, ctx))
    }
}
