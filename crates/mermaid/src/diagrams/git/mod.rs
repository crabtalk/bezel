//! `gitGraph`: commits on branches. Pele's `diagrams/git`.

mod db;
mod parser;
mod render;

pub use db::{GitCommit, GitModel};

use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};

use super::{Diagram, Model, Render};
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
        GitModel::new(parser::parse_git(source)?, &config.git_graph, title).map(Model::GitGraph)
    }
}

/// Mermaid's `gitGraph` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GitGraphConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub show_branches: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub show_commit_label: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub rotate_commit_label: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub parallel_commits: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub main_branch_name: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    pub main_branch_order: Option<f64>,
}

impl Render for GitModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(render::render_git(self, &config.git_graph, ctx))
    }
}
