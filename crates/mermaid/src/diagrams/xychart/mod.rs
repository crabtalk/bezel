//! `xychart`: lines and bars over two axes. Pele's `diagrams/xychart`.

pub mod db;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod ticks;
pub mod tokens;

pub use db::XyChartModel;

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};

pub struct XyChart;

impl Diagram for XyChart {
    fn section(&self) -> Option<&'static str> {
        Some("xyChart")
    }

    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = XyChartModel {
            title: title.map(str::to_string),
            ..XyChartModel::default()
        };
        parser::parse_xy_chart(source, &mut db)?;
        Ok(Model::XyChart(db))
    }
}

pub fn render(model: &Model, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
    let Model::XyChart(model) = model else {
        unreachable!("xychart renders an xychart model")
    };
    render::render_xy_chart(model, config, ctx)
}
