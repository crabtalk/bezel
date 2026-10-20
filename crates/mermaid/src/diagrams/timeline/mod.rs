//! `timeline`: periods along an axis, with their events. Pele's
//! `diagrams/timeline`.

pub mod db;
pub mod lexer;
pub mod parser;
pub mod render;

pub use db::{TimelineModel, TimelinePeriod};

use super::{Diagram, Model, Render};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};
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

impl Render for TimelineModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(render::render_timeline(self, &config.timeline, ctx))
    }
}
