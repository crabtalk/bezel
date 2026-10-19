//! `packet`: a network packet's fields, row by row. Pele's
//! `diagrams/packet`.

mod model;
mod parser;
mod render;

pub use model::{PacketBlock, PacketModel};

use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};

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
        model::build_packet(parser::parse_packet(source)?, &config.packet, title).map(Model::Packet)
    }
}

/// Mermaid's `packet` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PacketConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub bits_per_row: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub row_height: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub bit_width: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub padding_x: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub padding_y: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub show_bits: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub bit_order: Option<String>,
}

pub fn render(model: &Model, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
    let Model::Packet(model) = model else {
        unreachable!("packet renders a packet model")
    };
    Ok(render::render_packet(model, &config.packet, ctx))
}
