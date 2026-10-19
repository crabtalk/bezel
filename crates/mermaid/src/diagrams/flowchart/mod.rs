//! `flowchart` and `graph`: nodes and the links between them, in nested
//! subgraphs. Pele's `diagrams/flowchart`.

pub mod db;
pub mod graph;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod shapes;
pub mod tokens;
pub mod types;

use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};

pub use db::{FlowDb, FlowDbOptions};

pub struct Flowchart;

/// Mermaid's `flowchart` config, and a flowchart variant's own section.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FlowchartConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub curve: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    pub inherit_dir: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub wrapping_width: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub node_spacing: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub rank_spacing: Option<f64>,
}

impl Diagram for Flowchart {
    fn parse(
        &self,
        source: &str,
        config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let flow = &config.flowchart;
        let inherit_dir = flow.inherit_dir == Some(true);
        let mut db = FlowDb::new(FlowDbOptions {
            inherit_dir,
            max_edges: Some(limits.max_edges),
        });
        if let Some(title) = title.filter(|title| !title.is_empty()) {
            db.title = Some(title.to_string());
        }
        parser::parse_flowchart(source, &mut db)?;
        Ok(Model::Flowchart(Box::new(db)))
    }
}

pub fn render(model: &Model, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
    let Model::Flowchart(db) = model else {
        unreachable!("flowchart renders a flowchart model")
    };
    Ok(render::render_flowchart(db, &config.flowchart, ctx, None))
}
