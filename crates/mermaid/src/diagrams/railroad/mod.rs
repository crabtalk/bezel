//! `railroad`: grammar rules drawn as syntax diagrams, written in Mermaid's
//! own notation, EBNF, ABNF or PEG. Pele's `diagrams/railroad`.

pub mod abnf;
pub mod common;
pub mod db;
pub mod ebnf;
pub mod ir;
pub mod peg;
pub mod render;
pub mod types;

use super::{Diagram, Model, Render};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};
pub use render::RailroadConfig;

pub struct Railroad;

impl Diagram for Railroad {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        Ok(Model::Railroad(db::RailroadDb::parse(source, title)?))
    }
}

impl Render for db::RailroadDb {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(render::render_railroad(self, &config.railroad, ctx))
    }
}
