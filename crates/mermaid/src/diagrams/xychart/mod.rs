//! `xychart`: lines and bars over two axes. Pele's `diagrams/xychart`.

pub mod db;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod ticks;
pub mod tokens;

pub use db::XyChartModel;

use super::{Diagram, Model, Render};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};
pub use render::XyChartConfig;

pub struct XyChart;

impl Diagram for XyChart {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = XyChartModel::default();
        db.titles.title = title.map(str::to_string);
        parser::parse_xy_chart(source, &mut db)?;
        Ok(Model::XyChart(db))
    }
}

impl Render for XyChartModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        render::render_xy_chart(self, &config.xy_chart, ctx)
    }
}
