//! `quadrantChart`: points on two axes, in four quadrants. Pele's
//! `diagrams/quadrant`.

pub mod db;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod tokens;

pub use db::QuadrantModel;

use super::{Diagram, Model};
use crate::{Config, Error, Options, ParseLimits, Rendered};

pub struct Quadrant;

impl Diagram for Quadrant {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = QuadrantModel {
            title: title.map(str::to_string),
            ..QuadrantModel::default()
        };
        parser::parse_quadrant(source, &mut db)?;
        Ok(Model::Quadrant(db))
    }

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
        let Model::Quadrant(model) = model else {
            unreachable!("quadrantChart renders a quadrant model")
        };
        Ok(render::render_quadrant(model, config, options))
    }
}
