//! `block`: blocks on a grid, nested, with edges between them. Pele's
//! `diagrams/block`.

pub mod arrows;
pub mod db;
pub mod layout;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod tokens;
pub mod types;

pub use types::BlockModel;

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits};
pub use render::BlockConfig;

pub struct Block;

impl Diagram for Block {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = BlockModel::new(title);
        parser::parse_block(source, &mut db)?;
        Ok(Model::Block(Box::new(db)))
    }
}
