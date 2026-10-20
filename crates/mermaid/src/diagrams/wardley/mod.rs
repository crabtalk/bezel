//! `wardley`: a Wardley map, components placed by visibility and evolution.
//! Pele's `diagrams/wardley`.

pub mod builder;
pub mod model;
pub mod parser;
pub mod render;

pub use model::WardleyModel;

use super::{Diagram, Model, Render};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};
pub use render::WardleyConfig;

pub struct Wardley;

impl Diagram for Wardley {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        Ok(Model::Wardley(Box::new(WardleyModel::new(
            parser::parse_wardley(source)?,
            title,
        )?)))
    }
}

impl Render for WardleyModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(render::render_wardley(self, &config.wardley, ctx))
    }
}
