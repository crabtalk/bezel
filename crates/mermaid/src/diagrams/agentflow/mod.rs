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
pub mod tokens;
pub mod types;

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};
pub use render::AgentflowConfig;

pub use db::{AgentflowDb, AgentflowDbOptions};

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
            db.title = Some(title.to_string());
        }
        parser::parse_agentflow(source, &mut db, false)?;
        db.graph = render::agent_graph(&mut db, flow);
        Ok(Model::Agentflow(Box::new(db)))
    }
}

pub fn render(model: &Model, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
    let Model::Agentflow(db) = model else {
        unreachable!("agentflow renders an agentflow model")
    };
    Ok(render::render_agentflow(db, &config.agentflow, ctx))
}
