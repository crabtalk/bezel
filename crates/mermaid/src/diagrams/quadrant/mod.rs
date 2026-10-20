//! `quadrantChart`: points on two axes, in four quadrants. Pele's
//! `diagrams/quadrant`.

pub mod db;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod tokens;

pub use db::QuadrantModel;

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits};
pub use render::QuadrantConfig;

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
        let mut db = QuadrantModel::default();
        db.titles.title = title.map(str::to_string);
        parser::parse_quadrant(source, &mut db)?;
        Ok(Model::Quadrant(db))
    }
}
