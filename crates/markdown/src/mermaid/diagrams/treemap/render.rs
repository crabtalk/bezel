//! Pele's `diagrams/treemap/render.ts`.

use super::{
    format::formatter,
    layout::{Insets, layout_treemap},
    model::TreemapModel,
};
use crate::mermaid::{
    Config, Options, Rendered,
    config::Value,
    diagrams::common::{
        fit_text::fit_text,
        fit_width::{cap_width, title_room},
    },
    js::{self, Lazy},
    svg::{
        builder::{esc, esc_text, label_svg, num},
        root::{Accessible, svg_document},
        theme::{RADIUS, class_names, resolve_style, series_color},
    },
    text::{Style, decode_entities, layout_label},
};

const HEADER: f64 = 28.0;
const SIDE: f64 = 6.0;
const LINE: f64 = 1.25;

static RE_FILL: Lazy = Lazy::new(r#"[";]fill:"#, "");

fn option(config: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    match config
        .section("treemap")
        .and_then(|treemap| treemap.get(key))
    {
        Some(Value::Number(value)) if *value >= min => value.min(max),
        _ => fallback,
    }
}

pub fn render_treemap(model: &TreemapModel, config: &Config, options: &Options) -> Rendered {
    let p = crate::mermaid::PREFIX;
    let size = options.font_size.unwrap_or(16.0);
    let small = js::round(size * 0.875);
    let tiny = js::round(size * 0.75);
    let least = js::round(size * 0.5625);
    let measure = options.measurer();
    let treemap = config.section("treemap");
    let pad = options
        .padding
        .unwrap_or_else(|| option(config, "diagramPadding", 8.0, 0.0, 2000.0));

    // Mermaid sizes the diagram as ten times `nodeWidth` by ten times
    // `nodeHeight`.
    let width = js::round(cap_width(
        options,
        option(config, "nodeWidth", 64.0, 1.0, 400.0) * 10.0,
        200.0,
        2.0 * pad,
    ));
    let height = js::round(option(config, "nodeHeight", 40.0, 1.0, 400.0) * 10.0);
    let show_values = treemap.and_then(|t| t.get("showValues")) != Some(&Value::Bool(false));
    let format = formatter(
        treemap
            .and_then(|t| t.get("valueFormat"))
            .and_then(Value::as_str)
            .unwrap_or(","),
    );
    let (all, placed) = layout_treemap(
        &model.roots,
        width,
        height,
        &Insets {
            gap: option(config, "padding", 4.0, 0.0, 200.0),
            header: HEADER,
            side: SIDE,
        },
    );

    let title = layout_label(
        model.title.as_deref(),
        false,
        &*measure,
        size,
        title_room(options),
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 8.0
    } else {
        0.0
    };
    let left = pad + 0f64.max((title.width - width) / 2.0);
    let top = pad + title_height;

    let mut sections = String::new();
    let mut leaves = String::new();
    for &index in &placed {
        let b = &all[index];
        let node = b.node;
        let style = resolve_style(node.css_compiled_styles.as_deref().unwrap_or(&[]));
        let color = series_color(b.series as i64);
        let x = left + b.x;
        let y = top + b.y;
        let name = decode_entities(&node.name);
        let name = js::trim(&name);
        let value = if show_values {
            format.format(b.value)
        } else {
            String::new()
        };
        let group = format!("data-id=\"{}\">", esc_text(&node.name));
        let rect = format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"{color}\"",
            num(x),
            num(y),
            num(b.w),
            num(b.h)
        );
        let classes = class_names(node.class_selector.as_deref().unwrap_or(""));

        if node.children.is_some() {
            let mut text = String::new();
            if b.h >= HEADER {
                let room = b.w - 2.0 * SIDE - 4.0;
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
                    &*measure,
                    Style::BOLD,
                );
                let baseline = y + HEADER / 2.0 + 1.0;
                if let Some(label) = label {
                    text.push_str(&format!(
                        "<text x=\"{}\" y=\"{}\" font-weight=\"var(--_hw)\"{}>{}</text>",
                        num(x + SIDE + 2.0),
                        num(baseline + small * 0.35),
                        style.text,
                        esc(&label.text)
                    ));
                }
                if with_value {
                    text.push_str(&format!(
                        "<text class=\"{p}-value\" x=\"{}\" y=\"{}\" font-size=\"{}\" text-anchor=\"end\" fill=\"var(--_m)\"{}>{}</text>",
                        num(x + b.w - SIDE - 2.0),
                        num(baseline + tiny * 0.35),
                        js::num_str(tiny),
                        style.text,
                        esc(&value)
                    ));
                }
            }
            // A section is a tint of its color, unless a class gives it a
            // fill of its own.
            let tint = if RE_FILL.test(&style.shape) {
                ""
            } else {
                " fill-opacity=\"0.12\""
            };
            sections.push_str(&format!(
                "<g class=\"{p}-node {p}-section{classes}\" {group}{rect}{tint} stroke=\"{color}\"{}/>{text}</g>",
                style.shape
            ));
            continue;
        }

        let mut text = String::new();
        let room = b.w - 8.0;
        let tallest = size.min(((b.h - 4.0) / LINE).floor());
        let label = if tallest >= least {
            fit_text(name, room, tallest, least, &*measure, 0)
        } else {
            None
        };
        if let Some(label) = label {
            let cx = num(x + b.w / 2.0);
            let cy = y + b.h / 2.0;
            let value_size = least.max(js::round(label.size * 0.8));
            let lines = (label.size + value_size) * LINE;
            if !value.is_empty()
                && lines <= b.h - 4.0
                && measure.width(&value, value_size, 0) <= room
            {
                let first = cy - lines / 2.0 + (label.size * LINE) / 2.0;
                let second = cy + lines / 2.0 - (value_size * LINE) / 2.0;
                text = format!(
                    "<text x=\"{cx}\" y=\"{}\" font-size=\"{}\"{}>{}</text><text class=\"{p}-value\" x=\"{cx}\" y=\"{}\" font-size=\"{}\"{}>{}</text>",
                    num(first + label.size * 0.35),
                    num(label.size),
                    style.text,
                    esc(&label.text),
                    num(second + value_size * 0.35),
                    num(value_size),
                    style.text,
                    esc(&value)
                );
            } else {
                text = format!(
                    "<text x=\"{cx}\" y=\"{}\" font-size=\"{}\"{}>{}</text>",
                    num(cy + label.size * 0.35),
                    num(label.size),
                    style.text,
                    esc(&label.text)
                );
            }
        }
        leaves.push_str(&format!(
            "<g class=\"{p}-node {p}-leaf{classes}\" {group}{rect}{}/>{text}</g>",
            style.shape
        ));
    }

    let total_width = (width.max(title.width) + 2.0 * pad).ceil();
    let total_height = (height + title_height + 2.0 * pad).ceil();
    let mut content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
    );
    if !sections.is_empty() {
        content.push_str(&format!(
            "<g class=\"{p}-sections\" font-size=\"{}\">{sections}</g>",
            js::num_str(small)
        ));
    }
    if !leaves.is_empty() {
        content.push_str(&format!(
            "<g class=\"{p}-leaves\" text-anchor=\"middle\" fill=\"var(--_bg)\">{leaves}</g>"
        ));
    }

    Rendered {
        svg: svg_document(
            "treemap",
            total_width,
            total_height,
            size,
            options,
            Accessible {
                acc_title: model.acc_title.as_deref(),
                acc_descr: model.acc_descr.as_deref(),
            },
            &content,
        ),
        width: total_width,
        height: total_height,
        links: Vec::new(),
    }
}
