//! `kanban`: columns of cards. Pele's `diagrams/kanban`.

pub mod db;
pub mod parser;
pub mod render;

pub use db::{KanbanModel, KanbanNode, KanbanSection};

use super::{Diagram, Model};
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
        let mut db = KanbanModel::new();
        db.title = title.map(str::to_string);
        parser::parse_kanban(source, &mut db)?;
        Ok(Model::Kanban(db))
    }
}

pub fn render(model: &Model, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
    let Model::Kanban(model) = model else {
        unreachable!("kanban renders a kanban model")
    };
    Ok(render::render_kanban(model, &config.kanban, ctx))
}
