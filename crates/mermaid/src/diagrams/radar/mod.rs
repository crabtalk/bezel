//! `radar`: values on axes around a centre. Pele's `diagrams/radar`.

pub mod model;
pub mod parser;
pub mod render;

pub use model::RadarModel;

use super::{Diagram, Model, Render};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};
pub use render::RadarConfig;

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
        Ok(Model::Radar(RadarModel::new(
            parser::parse_radar(source)?,
            title,
        )?))
    }
}

impl Render for RadarModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(render::render_radar(self, &config.radar, ctx))
    }
}
