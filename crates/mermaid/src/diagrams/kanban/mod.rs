//! `kanban`: columns of cards. Pele's `diagrams/kanban`.

pub mod db;
pub mod parser;
pub mod render;

pub use db::{KanbanModel, KanbanNode, KanbanSection};

use super::{Diagram, Model, Render};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};
pub use render::KanbanConfig;

pub struct Kanban;

impl Diagram for Kanban {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = KanbanModel::default();
        db.title = title.map(str::to_string);
        parser::parse_kanban(source, &mut db)?;
        Ok(Model::Kanban(db))
    }
}

impl Render for KanbanModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(render::render_kanban(self, &config.kanban, ctx))
    }
}
