//! Pele's `diagrams/packet/model.ts`.

use serde::Serialize;

use super::parser::PacketAst;
use crate::{
    Config, Error, Kind, Value,
    js::{self, Lazy},
};

const MAX_ROWS: usize = 10_000;

static RE_LEAD: Lazy = Lazy::new(r"^\s+", "g");
static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_descr: Option<String>,
    #[serde(serialize_with = "js::json_number")]
    pub bits_per_row: f64,
    pub rows: Vec<Vec<PacketBlock>>,
}

pub fn packet_option(config: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    match config.section("packet").and_then(|packet| packet.get(key)) {
        Some(Value::Number(value)) if *value >= min => value.min(max),
        _ => fallback,
    }
}

fn invalid(message: String) -> Error {
    Error::semantic(message).of(Kind::Packet)
}

pub fn build_packet(
    ast: PacketAst,
    config: &Config,
    title: Option<&str>,
) -> Result<PacketModel, Error> {
    let bits_per_row = packet_option(config, "bitsPerRow", 32.0, 1.0, 4096.0).floor();
    let mut rows: Vec<Vec<PacketBlock>> = Vec::new();
    let mut last_bit = -1.0;
    let mut word: Vec<PacketBlock> = Vec::new();
    let mut row = 1.0;

    for block in &ast.blocks {
        let (start, end, bits) = (block.start, block.end, block.bits);
        if let (Some(start), Some(end)) = (start, end)
            && end < start
        {
            return Err(invalid(format!(
                "Packet block {} - {} is invalid. End must be greater than start.",
                js::num_str(start),
                js::num_str(end)
            )));
        }
        let mut start = start.unwrap_or(last_bit + 1.0);
        if start != last_bit + 1.0 {
            return Err(invalid(format!(
                "Packet block {} - {} is not contiguous. It should start from {}.",
                js::num_str(start),
                js::num_str(end.unwrap_or(start)),
                js::num_str(last_bit + 1.0)
            )));
        }
        if bits == Some(0.0) {
            return Err(invalid(format!(
                "Packet block {} is invalid. Cannot have a zero bit field.",
                js::num_str(start)
            )));
        }
        let end = end.unwrap_or(start + bits.unwrap_or(1.0) - 1.0);
        let mut bits = bits.unwrap_or(end - start + 1.0);
        last_bit = end;

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

    Ok(PacketModel {
        kind: "packet",
        title: ast
            .title
            .filter(|title| !title.is_empty())
            .or_else(|| title.map(str::to_string)),
        acc_title: ast
            .acc_title
            .filter(|text| !text.is_empty())
            .map(|text| RE_LEAD.replace(&text, |_| String::new())),
        acc_descr: ast
            .acc_descr
            .filter(|text| !text.is_empty())
            .map(|text| RE_INDENT.replace(&text, |_| "\n".into())),
        bits_per_row,
        rows,
    })
}
