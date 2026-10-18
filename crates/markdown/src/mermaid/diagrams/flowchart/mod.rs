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

use super::{Diagram, Model};
use crate::mermaid::{Config, Error, Options, ParseLimits, Rendered, config::Value};

pub use db::{FlowDb, FlowDbOptions};

pub struct Flowchart;

impl Diagram for Flowchart {
    fn parse(
        &self,
        source: &str,
        config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let inherit_dir = config
            .section("flowchart")
            .and_then(|flow| flow.get("inheritDir"))
            == Some(&Value::Bool(true));
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

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
        let Model::Flowchart(db) = model else {
            unreachable!("flowchart renders a flowchart model")
        };
        Ok(render::render_flowchart(db, config, options, None))
    }
}
