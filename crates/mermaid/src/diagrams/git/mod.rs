//! `gitGraph`: commits on branches. Pele's `diagrams/git`.

mod db;
mod parser;
mod render;

pub use db::{GitCommit, GitModel};

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};

pub struct GitGraph;

impl Diagram for GitGraph {
    fn parse(
        &self,
        source: &str,
        config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        db::build_git(parser::parse_git(source)?, config, title).map(Model::GitGraph)
    }
}

pub fn render(model: &Model, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
    let Model::GitGraph(model) = model else {
        unreachable!("gitGraph renders a gitGraph model")
    };
    Ok(render::render_git(model, config, ctx))
}
