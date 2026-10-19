//! `radar`: values on axes around a centre. Pele's `diagrams/radar`.

pub mod model;
pub mod parser;
pub mod render;

pub use model::RadarModel;

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};

pub struct Radar;

impl Diagram for Radar {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        Ok(Model::Radar(model::build_radar(
            parser::parse_radar(source)?,
            title,
        )?))
    }
}

pub fn render(model: &Model, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
    let Model::Radar(model) = model else {
        unreachable!("radar renders a radar model")
    };
    Ok(render::render_radar(model, config, ctx))
}
