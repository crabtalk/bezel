//! `agentflow`: the steps, tools and hand-offs of an agent, as nodes in
//! nested flows. Pele's `diagrams/agentflow`.

pub mod color_slots;
pub mod db;
pub mod diagnostics;
pub mod graph;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod shapes;
pub mod types;

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits};
pub use render::AgentflowConfig;

pub use db::{AgentflowDb, AgentflowDbOptions};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Agentflow;

impl Diagram for Agentflow {
    fn keep_comments(&self) -> bool {
        true
    }

    fn parse(
        &self,
        source: &str,
        config: &Config,
        title: Option<&str>,
        line_offset: usize,
        limits: &ParseLimits,
    ) -> Result<Model, Error> {
        // Mermaid reads this setting from the flowchart section for
        // agentflow too.
        let flow = &config.flowchart;
        let inherit_dir = flow.inherit_dir == Some(true);
        let mut db = AgentflowDb::new(AgentflowDbOptions {
            inherit_dir,
            direction: config.direction.clone(),
            line_offset,
            max_edges: Some(limits.max_edges),
        });
        if let Some(title) = title.filter(|title| !title.is_empty()) {
            db.store.titles.set_title(title);
        }
        parser::parse_agentflow(source, &mut db, false)?;
        db.graph = db.build_graph(render::PALETTE, flow.curve.as_deref());
        Ok(Model::Agentflow(Box::new(db)))
    }
}
