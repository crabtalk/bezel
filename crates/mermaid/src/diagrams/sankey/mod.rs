//! `sankey`: flows between nodes, as bands of their size. Pele's
//! `diagrams/sankey`.

pub mod db;
pub mod layout;
pub mod lexer;
pub mod parser;
pub mod render;

pub use db::SankeyModel;

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits};
pub use render::SankeyConfig;

pub struct Sankey;

impl Diagram for Sankey {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = SankeyModel::default();
        if let Some(title) = title.filter(|title| !title.is_empty()) {
            db.titles.set_title(title);
        }
        parser::parse_sankey(&parser::prepare_text_for_parsing(source), &mut db)?;
        Ok(Model::Sankey(db))
    }
}
