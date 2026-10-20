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

use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits};

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
            db.titles.title = Some(title.to_string());
        }
        if let Some(mode) = &config.gantt.display_mode {
            db.display_mode = mode.clone();
        }
        parser::parse_gantt(source, &mut db)?;
        Ok(Model::Gantt(Box::new(db)))
    }
}

/// Mermaid's `gantt` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GanttConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub font_size: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub section_font_size: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub bar_height: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub bar_gap: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub number_section_styles: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub left_padding: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub right_padding: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub use_width: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub top_padding: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub grid_line_start_padding: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub top_axis: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub axis_format: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    pub tick_interval: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    pub weekday: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    pub display_mode: Option<String>,
}
