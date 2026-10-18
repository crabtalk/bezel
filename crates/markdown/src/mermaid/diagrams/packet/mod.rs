//! `packet`: a network packet's fields, row by row. Pele's
//! `diagrams/packet`.

mod model;
mod parser;
mod render;

pub use model::{PacketBlock, PacketModel};

use super::{Diagram, Model};
use crate::mermaid::{Config, Error, Options, ParseLimits, Rendered};

pub struct Packet;

impl Diagram for Packet {
    fn parse(
        &self,
        source: &str,
        config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        model::build_packet(parser::parse_packet(source)?, config, title).map(Model::Packet)
    }

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
        let Model::Packet(model) = model else {
            unreachable!("packet renders a packet model")
        };
        Ok(render::render_packet(model, config, options))
    }
}
