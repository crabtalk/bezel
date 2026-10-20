//! Pele's `diagrams/packet/model.ts`.

use super::PacketConfig;
use crate::config::capped;
use crate::diagrams::common::db::Titles;
use serde::Serialize;

use super::parser::PacketAst;
use crate::{
    Error, Kind,
    js::{self},
};

const MAX_ROWS: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PacketBlock {
    #[serde(serialize_with = "js::json_number")]
    pub start: f64,
    #[serde(serialize_with = "js::json_number")]
    pub end: f64,
    /// As Mermaid counts it: one less than the width for the pieces of a
    /// field split across rows.
    #[serde(serialize_with = "js::json_number")]
    pub bits: f64,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PacketModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub titles: Titles,
    #[serde(serialize_with = "js::json_number")]
    pub bits_per_row: f64,
    pub rows: Vec<Vec<PacketBlock>>,
}

impl PacketModel {
    pub fn new(ast: PacketAst, config: &PacketConfig, title: Option<&str>) -> Result<Self, Error> {
        let bits_per_row = capped(config.bits_per_row, 32.0, 1.0, 4096.0).floor();
        let mut rows: Vec<Vec<PacketBlock>> = Vec::new();
        // The bit the next block must start at.
        let mut next_bit = 0.0;
        let mut word: Vec<PacketBlock> = Vec::new();
        let mut row = 1.0;

        for block in &ast.blocks {
            let (start, end, bits) = (block.start, block.end, block.bits);
            if let (Some(start), Some(end)) = (start, end)
                && end < start
            {
                return Err(Error::semantic(format!(
                    "Packet block {} - {} is invalid. End must be greater than start.",
                    js::num_str(start),
                    js::num_str(end)
                ))
                .of(Kind::Packet));
            }
            let mut start = start.unwrap_or(next_bit);
            if start != next_bit {
                return Err(Error::semantic(format!(
                    "Packet block {} - {} is not contiguous. It should start from {}.",
                    js::num_str(start),
                    js::num_str(end.unwrap_or(start)),
                    js::num_str(next_bit)
                ))
                .of(Kind::Packet));
            }
            if bits == Some(0.0) {
                return Err(Error::semantic(format!(
                    "Packet block {} is invalid. Cannot have a zero bit field.",
                    js::num_str(start)
                ))
                .of(Kind::Packet));
            }
            let end = end.unwrap_or(start + bits.unwrap_or(1.0) - 1.0);
            let mut bits = bits.unwrap_or(end - start + 1.0);
            next_bit = end + 1.0;

            while word.len() as f64 <= bits_per_row + 1.0 && rows.len() < MAX_ROWS {
                let row_end = row * bits_per_row - 1.0;
                if end <= row_end {
                    word.push(PacketBlock {
                        start,
                        end,
                        bits,
                        label: block.label.clone(),
                    });
                    if end == row_end {
                        rows.push(std::mem::take(&mut word));
                        row += 1.0;
                    }
                    break;
                }
                word.push(PacketBlock {
                    start,
                    end: row_end,
                    bits: row_end - start,
                    label: block.label.clone(),
                });
                rows.push(std::mem::take(&mut word));
                row += 1.0;
                start = row_end + 1.0;
                bits = end - start;
            }
        }
        if !word.is_empty() {
            rows.push(word);
        }

        Ok(Self {
            kind: "packet",
            titles: ast.header.titles(title),
            bits_per_row,
            rows,
        })
    }
}
