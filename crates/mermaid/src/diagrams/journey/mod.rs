//! `journey`: a user journey, tasks scored by how they felt. Pele's
//! `diagrams/journey`.

pub mod db;
pub mod lexer;
pub mod parser;
pub mod render;

pub use db::{JourneyModel, JourneyTask};

use super::{Diagram, Model};
use crate::{Config, Error, Options, ParseLimits, Rendered};

pub struct Journey;

impl Diagram for Journey {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = JourneyModel::new();
        db.title = title.map(str::to_string);
        parser::parse_journey(source, &mut db)?;
        Ok(Model::Journey(db))
    }

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
        let Model::Journey(model) = model else {
            unreachable!("journey renders a journey model")
        };
        Ok(render::render_journey(model, config, options))
    }
}
