//! Pele's `diagrams/packet/render.ts`.

use crate::geom::{Point, Size};
use crate::svg::builder::Esc;
use std::{collections::HashMap, fmt::Write as _};

use super::{
    BitOrder, PacketConfig,
    model::{PacketBlock, PacketModel},
};
use crate::config::capped;
use crate::{Config, Ctx, Error, Host};
use crate::{
    Rendered,
    diagrams::{Render, common::fit_text::fit_text},
    js,
    svg::{
        builder::{TITLE, fragment, num},
        root::svg_document,
        theme::RADIUS,
    },
    text::{Label, Style, decode_entities},
};

impl Render for PacketModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(ctx.turn_to_fit(|down| self.draw(&config.packet, ctx, down)))
    }
}

impl PacketModel {
    fn draw(&self, config: &PacketConfig, ctx: &Ctx<'_, impl Host>, down: bool) -> Rendered {
        let p = crate::PREFIX;
        let size = ctx.options.font_size;
        let label_size = js::round(size * 0.875);
        let bit_size = js::round(size * 0.625);
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);

        let row_height = capped(config.row_height, 32.0, 1.0, 400.0);
        let mut bit_width = capped(config.bit_width, 32.0, 1.0, 400.0);
        let padding_x = capped(config.padding_x, 5.0, 0.0, 400.0);
        let padding_y = capped(config.padding_y, 5.0, 0.0, 400.0);
        let show_bits = config.show_bits != Some(false);
        let descending = config.bit_order == Some(BitOrder::Descending);
        let mut bits_per_row = self.bits_per_row;
        let mut rows: Vec<Vec<PacketBlock>> = self.rows.clone();
        if down {
            let max_width = ctx.options.max_width.unwrap_or(320.0);
            let count = 1f64.max(((max_width - 2.0 * pad) / bit_width.min(18.0)).floor());
            // A byte is the shortest row. Below that every label is one character
            // to a line, and a long packet is a row to a bit.
            bits_per_row = bits_per_row.min(8f64.max(2f64.powf(count.log2().floor())));
            let last = self
                .rows
                .last()
                .and_then(|row| row.last())
                .map_or(0.0, |block| block.end);
            if ((last + 1.0) / bits_per_row).ceil() > 10000.0 {
                bits_per_row = self.bits_per_row;
            }
            bit_width = bit_width.min(18f64.max((max_width - 2.0 * pad) / bits_per_row));
            // The self has split each field at its own rows. Its pieces are
            // joined again first, so that a field is only split where one of the
            // new rows ends. A piece counts one bit short.
            let piece = |block: &PacketBlock| block.bits == block.end - block.start;
            let mut whole: Vec<PacketBlock> = Vec::new();
            for block in self.rows.iter().flatten() {
                if let Some(prev) = whole.last_mut()
                    && piece(prev)
                    && piece(block)
                    && prev.label == block.label
                    && block.start == prev.end + 1.0
                    && block.start % self.bits_per_row == 0.0
                {
                    prev.end = block.end;
                    prev.bits = prev.end - prev.start;
                } else {
                    whole.push(block.clone());
                }
            }
            rows = Vec::new();
            for block in &whole {
                let mut start = block.start;
                while start <= block.end {
                    let end = block
                        .end
                        .min(((start / bits_per_row).floor() + 1.0) * bits_per_row - 1.0);
                    let index = (start / bits_per_row).floor() as usize;
                    if rows.len() <= index {
                        rows.resize_with(index + 1, Vec::new);
                    }
                    rows[index].push(PacketBlock {
                        start,
                        end,
                        ..block.clone()
                    });
                    start = end + 1.0;
                }
            }
        }
        let labels: Vec<Vec<Label>> = if down {
            rows.iter()
                .map(|row| {
                    row.iter()
                        .map(|block| {
                            let room = 1f64.max(
                                (block.end - block.start + 1.0) * bit_width
                                    - padding_x.min(bit_width - 1.0)
                                    - 6.0,
                            );
                            Label::literal(
                                js::trim(&decode_entities(&block.label)),
                                &*measure,
                                label_size,
                                room,
                                0,
                            )
                        })
                        .collect()
                })
                .collect()
        } else {
            Vec::new()
        };
        // Bit numbers sit above each row and need room of their own.
        let numbers = if show_bits { bit_size + 4.0 } else { 0.0 };
        let row_heights: Vec<f64> = rows
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let mut height = row_height;
                if down {
                    for label in &labels[index] {
                        height = height.max(label.height + 10.0);
                    }
                }
                height
            })
            .collect();
        let mut tops = Vec::with_capacity(rows.len());
        let mut rows_height = 0.0;
        for height in &row_heights {
            tops.push(rows_height);
            rows_height += numbers + height + padding_y;
        }
        if !rows.is_empty() {
            rows_height -= padding_y;
        }

        let gap = padding_x.min(bit_width - 1.0);
        let width = if rows.is_empty() {
            0.0
        } else {
            bits_per_row * bit_width - gap
        };
        let title = Label::layout(
            self.titles.title.as_deref(),
            false,
            measure,
            size,
            if down { ctx.title_room() } else { 4000.0 },
            Style::BOLD,
        );
        let title_height = if title.height > 0.0 {
            title.height + if rows_height > 0.0 { 12.0 } else { 0.0 }
        } else {
            0.0
        };

        // The pieces of a field split across rows share its label, so each width
        // is fitted once.
        let mut fits: HashMap<(String, u64), Option<(String, f64)>> = HashMap::new();
        let mut fit = |label: &str, max: f64| -> Option<(String, f64)> {
            fits.entry((label.to_string(), max.to_bits()))
                .or_insert_with(|| {
                    fit_text(
                        js::trim(&decode_entities(label)),
                        max,
                        label_size,
                        js::round(size * 0.5),
                        &*measure,
                        0,
                    )
                    .map(|fitted| (fitted.text, fitted.size))
                })
                .clone()
        };

        let left = pad + 0f64.max((title.width - width) / 2.0);
        let mut fields = String::new();
        let mut bits = String::new();
        for (index, row) in rows.iter().enumerate() {
            let y = pad + tops[index] + numbers;
            for (at, block) in row.iter().enumerate() {
                let count = block.end - block.start + 1.0;
                let first = block.start % bits_per_row;
                let corner = Point::new(
                    left + if descending {
                        bits_per_row - first - count
                    } else {
                        first
                    } * bit_width,
                    y,
                );
                let field = Size::new(count * bit_width - gap, row_heights[index]);
                let center = Point::new(corner.x + field.w / 2.0, corner.y + field.h / 2.0);
                let _ = write!(
                    fields,
                    "<g class=\"{p}-node\" data-id=\"{}-{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"/>",
                    js::Number(block.start),
                    js::Number(block.end),
                    num(corner.x),
                    num(corner.y),
                    num(field.w),
                    num(field.h)
                );
                if down {
                    let _ = write!(fields, "{}", labels[index][at].svg(center, ""));
                } else if let Some((text, text_size)) = fit(&block.label, field.w - 6.0)
                    && text_size <= field.h
                {
                    let font = fragment(|f| {
                        if text_size == label_size {
                            Ok(())
                        } else {
                            write!(f, " font-size=\"{}\"", num(text_size))
                        }
                    });
                    let _ = write!(
                        fields,
                        "<text x=\"{}\" y=\"{}\"{font}>{}</text>",
                        num(center.x),
                        num(center.y + text_size * 0.35),
                        Esc(&text)
                    );
                }
                fields.push_str("</g>");
                if !show_bits {
                    continue;
                }
                let leading = if descending { block.end } else { block.start };
                let trailing = if descending { block.start } else { block.end };
                if count == 1.0 {
                    let _ = write!(
                        bits,
                        "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
                        num(center.x),
                        num(corner.y - 4.0),
                        js::Number(leading)
                    );
                } else {
                    let _ = write!(
                        bits,
                        "<text x=\"{}\" y=\"{}\">{}</text><text x=\"{}\" y=\"{}\" text-anchor=\"end\">{}</text>",
                        num(corner.x),
                        num(corner.y - 4.0),
                        js::Number(leading),
                        num(corner.x + field.w),
                        num(corner.y - 4.0),
                        js::Number(trailing)
                    );
                }
            }
        }

        let total_width = (width.max(title.width) + 2.0 * pad).ceil();
        let total_height = (rows_height + title_height + 2.0 * pad).ceil();
        let mut content = String::with_capacity(fields.len() + bits.len() + 512);
        if !fields.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-packet-fields\" font-size=\"{}\" text-anchor=\"middle\">{fields}</g>",
                js::Number(label_size)
            );
        }
        if !bits.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-packet-bits\" font-size=\"{}\" fill=\"var(--_m)\">{bits}</g>",
                js::Number(bit_size)
            );
        }
        // Mermaid puts a packet's title under it.
        let _ = write!(
            content,
            "{}",
            title.svg(
                Point::new(total_width / 2.0, total_height - pad - title.height / 2.0),
                TITLE
            )
        );

        Rendered {
            svg: svg_document(
                "packet",
                total_width,
                total_height,
                size,
                ctx,
                self.titles.accessible(),
                &content,
            ),
            width: total_width,
            height: total_height,
            links: Vec::new(),
        }
    }
}
