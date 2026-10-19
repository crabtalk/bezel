//! `pie`: slices of a whole. Pele's `diagrams/pie`.

pub mod model;
pub mod parser;
pub mod render;

pub use model::PieModel;

use super::{Diagram, Model};
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
        Ok(Model::Pie(model::build_pie(
            parser::parse_pie(source)?,
            title,
        )?))
    }
}

pub fn render(model: &Model, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
    let Model::Pie(model) = model else {
        unreachable!("pie renders a pie model")
    };
    Ok(render::render_pie(model, &config.pie, ctx))
}
