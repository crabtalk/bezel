//! `mindmap`: an outline drawn as branches around its root. Pele's
//! `diagrams/mindmap`.

mod db;
pub mod parser;
mod render;

pub use db::{MindmapModel, MindmapNode};

use super::{Diagram, Model, Render};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};
pub use render::MindmapConfig;

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
}

impl Render for MindmapModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(render::render_mindmap(self, &config.mindmap, ctx))
    }
}
