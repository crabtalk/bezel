//! `gantt`: tasks on a time axis. Pele's `diagrams/gantt`.

pub mod axis;
pub mod dates;
pub mod db;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod tokens;
pub mod types;

pub use db::GanttDb;
pub use types::{GanttTask, RawTask};

use super::{Diagram, Model};
use crate::{Config, Error, Options, ParseLimits, Rendered, config::Value};

pub struct Gantt;

impl Diagram for Gantt {
    fn parse(
        &self,
        source: &str,
        config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = GanttDb::new();
        if let Some(title) = title.filter(|title| !title.is_empty()) {
            db.title = Some(title.to_string());
        }
        if let Some(Value::String(mode)) = config
            .section("gantt")
            .and_then(|gantt| gantt.get("displayMode"))
        {
            db.display_mode = mode.clone();
        }
        parser::parse_gantt(source, &mut db)?;
        Ok(Model::Gantt(Box::new(db)))
    }

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
        let Model::Gantt(model) = model else {
            unreachable!("gantt renders a gantt model")
        };
        render::render_gantt(model, config, options)
    }
}
