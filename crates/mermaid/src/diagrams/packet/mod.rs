//! `packet`: a network packet's fields, row by row. Pele's
//! `diagrams/packet`.

mod model;
mod parser;
mod render;

pub use model::{PacketBlock, PacketModel};

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits, Rendered};
use crate::{Ctx, Host};

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
}

pub fn render(model: &Model, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
    let Model::Packet(model) = model else {
        unreachable!("packet renders a packet model")
    };
    Ok(render::render_packet(model, config, ctx))
}
