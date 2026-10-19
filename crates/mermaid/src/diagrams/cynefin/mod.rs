//! `cynefin`: the five domains of the Cynefin framework. Pele's
//! `diagrams/cynefin`.

pub mod boundaries;
pub mod model;
pub mod parser;
pub mod render;

pub use model::CynefinModel;

use super::{Diagram, Model};
use crate::{Config, Error, Options, ParseLimits, Rendered};

pub struct Cynefin;

impl Diagram for Cynefin {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        Ok(Model::Cynefin(model::build_cynefin(
            parser::parse_cynefin(source)?,
            title,
        )))
    }

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
        let Model::Cynefin(model) = model else {
            unreachable!("cynefin renders a cynefin model")
        };
        Ok(render::render_cynefin(model, config, options))
    }
}
