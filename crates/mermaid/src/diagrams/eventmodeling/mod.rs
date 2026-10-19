//! `eventmodeling`: frames on a timeline in swim lanes, with their data,
//! notes and specifications. Pele's `diagrams/eventmodeling`.

pub mod model;
pub mod parser;
pub mod render;

pub use model::EventModelingModel;

use super::{Diagram, Model};
use crate::{Config, Error, Options, ParseLimits, Rendered};

pub struct EventModeling;

impl Diagram for EventModeling {
    fn keep_comments(&self) -> bool {
        true
    }

    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        Ok(Model::EventModeling(Box::new(model::build_event_model(
            parser::parse_event_model(source)?,
            title,
        )?)))
    }

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
        let Model::EventModeling(model) = model else {
            unreachable!("eventmodeling renders an eventmodeling model")
        };
        Ok(render::render_event_model(model, config, options))
    }
}
