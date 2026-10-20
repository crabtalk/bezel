//! `classDiagram`: classes, their members and the relations between them.
//! Pele's `diagrams/class`.

pub mod db;
pub mod graph;
pub mod lexer;
pub mod markers;
pub mod members;
pub mod parser;
pub mod render;
pub mod tokens;
pub mod types;

pub use db::ClassModel;

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits};
pub use render::ClassConfig;

pub struct Class;

impl Diagram for Class {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = ClassModel::default();
        if let Some(title) = title.filter(|title| !title.is_empty()) {
            db.titles.set_title(title);
        }
        parser::parse_class_diagram(source, &mut db)?;
        Ok(Model::Class(Box::new(db)))
    }
}
