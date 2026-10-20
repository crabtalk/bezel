//! `timeline`: periods along an axis, with their events. Pele's
//! `diagrams/timeline`.

pub mod db;
pub mod lexer;
pub mod parser;
pub mod render;

pub use db::{TimelineModel, TimelinePeriod};

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits};
pub use render::TimelineConfig;

pub struct Timeline;

impl Diagram for Timeline {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = TimelineModel::new();
        db.titles.title = title.map(str::to_string);
        parser::parse_timeline(source, &mut db)?;
        Ok(Model::Timeline(db))
    }
}
