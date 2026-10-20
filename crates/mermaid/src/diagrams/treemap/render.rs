//! Pele's `diagrams/treemap/render.ts`.

use crate::geom::{Point, Size};
use crate::text::Label;
use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};

use super::{format::Formatter, layout::Insets, model::TreemapModel};
use crate::config::capped;
use crate::svg::builder::{Esc, EscText};
use crate::{Config, Ctx, Error, Host};
use crate::{
    Rendered,
    diagrams::{Render, common::fit_text::fit_text},
    js::{self, Lazy},
    svg::{
        builder::{TITLE, fragment, num},
        root::svg_document,
        theme::{RADIUS, class_names, resolve_style, series_color},
    },
    text::{Style, decode_entities},
};
use std::fmt::Write as _;

const HEADER: f64 = 28.0;
const SIDE: f64 = 6.0;
const LINE: f64 = 1.25;

static RE_FILL: Lazy = Lazy::new(r#"[";]fill:"#, "");

/// Mermaid's `treemap` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TreemapConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub diagram_padding: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub padding: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub node_width: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub node_height: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub show_values: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub value_format: Option<String>,
}

impl Render for TreemapModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(self.draw(&config.treemap, ctx))
    }
}

impl TreemapModel {
    fn draw(&self, config: &TreemapConfig, ctx: &Ctx<'_, impl Host>) -> Rendered {
        let p = crate::PREFIX;
        let size = ctx.options.font_size;
        let small = js::round(size * 0.875);
        let tiny = js::round(size * 0.75);
        let least = js::round(size * 0.5625);
        let measure = ctx.host;
        let pad = ctx
            .options
            .padding
            .unwrap_or_else(|| capped(config.diagram_padding, 8.0, 0.0, 2000.0));

        // Mermaid sizes the diagram as ten times `nodeWidth` by ten times
        // `nodeHeight`.
        let width = js::round(ctx.cap_width(
            capped(config.node_width, 64.0, 1.0, 400.0) * 10.0,
            200.0,
            2.0 * pad,
        ));
        let height = js::round(capped(config.node_height, 40.0, 1.0, 400.0) * 10.0);
        let show_values = config.show_values != Some(false);
        let format = Formatter::new(config.value_format.as_deref().unwrap_or(","));
        let (all, placed) = self.layout(
            Size::new(width, height),
            &Insets {
                gap: capped(config.padding, 4.0, 0.0, 200.0),
                header: HEADER,
                side: SIDE,
            },
        );

        let title = Label::layout(
            self.titles.title.as_deref(),
            false,
            measure,
            size,
            ctx.title_room(),
            Style::BOLD,
        );
        let title_height = title.spaced_height(8.0);
        let left = pad + 0f64.max((title.width - width) / 2.0);
        let top = pad + title_height;

        let mut sections = String::new();
        let mut leaves = String::new();
        for &index in &placed {
            let b = &all[index];
            let node = b.node;
            let style = resolve_style(node.css_compiled_styles.as_deref().unwrap_or(&[]));
            let color = series_color(b.series as i64);
            let x = left + b.at.x;
            let y = top + b.at.y;
            let Size { w, h } = b.size;
            let name = decode_entities(&node.name);
            let name = js::trim(&name);
            let value = if show_values {
                format.format(b.value)
            } else {
                String::new()
            };
            let group = EscText(&node.name);
            let rect = fragment(|f| {
                write!(
                    f,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"{color}\"",
                    num(x),
                    num(y),
                    num(w),
                    num(h)
                )
            });
            let classes = class_names(node.class_selector.as_deref().unwrap_or(""));

            if node.children.is_some() {
                // A section is a tint of its color, unless a class gives it a
                // fill of its own.
                let tint = if RE_FILL.test(&style.shape) {
                    ""
                } else {
                    " fill-opacity=\"0.12\""
                };
                let _ = write!(
                    sections,
                    "<g class=\"{p}-node {p}-section{classes}\" data-id=\"{group}\">{rect}{tint} stroke=\"{color}\"{}/>",
                    style.shape
                );
                let text = &mut sections;
                if h >= HEADER {
                    let room = w - 2.0 * SIDE - 4.0;
                    let value_width = if value.is_empty() {
                        0.0
                    } else {
                        measure.width(&value, tiny, 0)
                    };
                    let with_value = value_width > 0.0 && room - value_width - 12.0 >= 3.0 * small;
                    let label = fit_text(
                        name,
                        if with_value {
                            room - value_width - 12.0
                        } else {
                            room
                        },
                        small,
                        small,
                        measure,
                        Style::BOLD,
                    );
                    let baseline = y + HEADER / 2.0 + 1.0;
                    if let Some(label) = label {
                        let _ = write!(
                            text,
                            "<text x=\"{}\" y=\"{}\" font-weight=\"var(--_hw)\"{}>{}</text>",
                            num(x + SIDE + 2.0),
                            num(baseline + small * 0.35),
                            style.text,
                            Esc(&label.text)
                        );
                    }
                    if with_value {
                        let _ = write!(
                            text,
                            "<text class=\"{p}-value\" x=\"{}\" y=\"{}\" font-size=\"{}\" text-anchor=\"end\" fill=\"var(--_m)\"{}>{}</text>",
                            num(x + w - SIDE - 2.0),
                            num(baseline + tiny * 0.35),
                            js::Number(tiny),
                            style.text,
                            Esc(&value)
                        );
                    }
                }
                sections.push_str("</g>");
                continue;
            }

            let _ = write!(
                leaves,
                "<g class=\"{p}-node {p}-leaf{classes}\" data-id=\"{group}\">{rect}{}/>",
                style.shape
            );
            let text = &mut leaves;
            let room = w - 8.0;
            let tallest = size.min(((h - 4.0) / LINE).floor());
            let label = if tallest >= least {
                fit_text(name, room, tallest, least, measure, 0)
            } else {
                None
            };
            if let Some(label) = label {
                let cx = num(x + w / 2.0);
                let cy = y + h / 2.0;
                let value_size = least.max(js::round(label.size * 0.8));
                let lines = (label.size + value_size) * LINE;
                if !value.is_empty()
                    && lines <= h - 4.0
                    && measure.width(&value, value_size, 0) <= room
                {
                    let first = cy - lines / 2.0 + (label.size * LINE) / 2.0;
                    let second = cy + lines / 2.0 - (value_size * LINE) / 2.0;
                    let _ = write!(
                        text,
                        "<text x=\"{cx}\" y=\"{}\" font-size=\"{}\"{}>{}</text><text class=\"{p}-value\" x=\"{cx}\" y=\"{}\" font-size=\"{}\"{}>{}</text>",
                        num(first + label.size * 0.35),
                        num(label.size),
                        style.text,
                        Esc(&label.text),
                        num(second + value_size * 0.35),
                        num(value_size),
                        style.text,
                        Esc(&value)
                    );
                } else {
                    let _ = write!(
                        text,
                        "<text x=\"{cx}\" y=\"{}\" font-size=\"{}\"{}>{}</text>",
                        num(cy + label.size * 0.35),
                        num(label.size),
                        style.text,
                        Esc(&label.text)
                    );
                }
            }
            leaves.push_str("</g>");
        }

        let total_width = (width.max(title.width) + 2.0 * pad).ceil();
        let total_height = (height + title_height + 2.0 * pad).ceil();
        let mut content = String::with_capacity(sections.len() + leaves.len() + 512);
        let _ = write!(
            content,
            "{}",
            title.svg(
                Point::new(total_width / 2.0, pad + title.height / 2.0),
                TITLE
            )
        );
        if !sections.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-sections\" font-size=\"{}\">{sections}</g>",
                js::Number(small)
            );
        }
        if !leaves.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-leaves\" text-anchor=\"middle\" fill=\"var(--_bg)\">{leaves}</g>"
            );
        }

        Rendered {
            svg: svg_document(
                "treemap",
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
