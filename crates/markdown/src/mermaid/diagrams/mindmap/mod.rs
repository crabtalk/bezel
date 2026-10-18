//! `mindmap`: an outline drawn as branches around its root. Pele's
//! `diagrams/mindmap`.

mod db;
pub mod parser;
mod render;

pub use db::{MindmapModel, MindmapNode};

use super::{Diagram, Model};
use crate::mermaid::{Config, Error, Options, ParseLimits, Rendered};

pub struct Mindmap;

impl Diagram for Mindmap {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = MindmapModel::new(title);
        parser::parse_mindmap(source, &mut db)?;
        Ok(Model::Mindmap(db))
    }

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
        let Model::Mindmap(model) = model else {
            unreachable!("mindmap renders a mindmap model")
        };
        Ok(render::render_mindmap(model, config, options))
    }
}
