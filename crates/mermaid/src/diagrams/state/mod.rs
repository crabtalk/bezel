//! `state`: states and the transitions between them, composites and their
//! regions, forks, joins, choices and notes. Pele's `diagrams/state`.

pub mod db;
pub mod graph;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod tokens;
pub mod types;

pub use types::StateModel;

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};

pub struct State;

impl Diagram for State {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let mut db = StateModel::new();
        if let Some(title) = title.filter(|title| !title.is_empty()) {
            db.title = Some(title.to_string());
        }
        parser::parse_state(source, &mut db)?;
        Ok(Model::State(Box::new(db)))
    }
}

pub fn render(model: &Model, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
    let Model::State(model) = model else {
        unreachable!("state renders a state model")
    };
    Ok(render::render_state(model, config, ctx))
}
