//! `cynefin`: the five domains of the Cynefin framework. Pele's
//! `diagrams/cynefin`.

pub mod boundaries;
pub mod model;
pub mod parser;
pub mod render;

pub use model::CynefinModel;

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits};
pub use render::CynefinConfig;

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
        Ok(Model::Cynefin(CynefinModel::new(
            parser::parse_cynefin(source)?,
            title,
        )))
    }
}
