//! `wardley`: a Wardley map, components placed by visibility and evolution.
//! Pele's `diagrams/wardley`.

pub mod builder;
pub mod model;
pub mod parser;
pub mod render;

pub use model::WardleyModel;

use super::{Diagram, Model};
use crate::{Config, Error, Options, ParseLimits, Rendered};

pub struct Wardley;

impl Diagram for Wardley {
    fn section(&self) -> Option<&'static str> {
        Some("wardley-beta")
    }

    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        Ok(Model::Wardley(Box::new(model::build_wardley(
            parser::parse_wardley(source)?,
            title,
        )?)))
    }

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
        let Model::Wardley(model) = model else {
            unreachable!("wardley renders a wardley model")
        };
        Ok(render::render_wardley(model, config, options))
    }
}
