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

use super::{Diagram, Model};
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
        Ok(Model::Railroad(db::parse_railroad(source, title)?))
    }
}

pub fn render(model: &Model, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
    match model {
        Model::Railroad(model) => Ok(render::render_railroad(model, &config.railroad, ctx)),
        _ => unreachable!("railroad renders a railroad model"),
    }
}
