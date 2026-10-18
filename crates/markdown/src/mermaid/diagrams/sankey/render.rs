//! Pele's `diagrams/sankey/render.ts`.

use std::collections::HashMap;

use super::{
    db::SankeyModel,
    layout::{Alignment, LayoutOptions, layout_sankey},
};
use crate::mermaid::{
    Config, Error, Options, Rendered,
    config::{Map, Value},
    diagrams::common::fit_width::{title_room, try_fit_width},
    js,
    svg::{
        builder::{esc, esc_text, label_svg, num},
        root::{Accessible, svg_document},
        theme::{resolve_style, series_color},
    },
    text::{Style, layout_label},
};

const LABEL_GAP: f64 = 6.0;
const VALUE_GAP: f64 = 15.0;

fn text_option(conf: &Map, key: &str) -> String {
    match conf.get(key) {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(value)) if value.is_finite() => js::num_str(*value),
        _ => String::new(),
    }
}

/// Two decimals, as Mermaid shows values; numbers too large for that are
/// left as they are.
fn rounded(value: f64) -> f64 {
    if value.abs() < 1e15 {
        js::round(value * 100.0) / 100.0
    } else {
        value
    }
}

/// A colour from the configuration, as an inline style that overrides the
/// series colour.
fn fill_style(color: Option<&Value>) -> String {
    match color {
        Some(Value::String(color)) => resolve_style(&[format!("fill:{color}")]).shape,
        _ => String::new(),
    }
}

pub fn render_sankey(
    model: &SankeyModel,
    config: &Config,
    options: &Options,
) -> Result<Rendered, Error> {
    let empty = Map::new();
    let conf = config.section("sankey").unwrap_or(&empty);
    try_fit_width(
        options,
        conf.number_in("width", 600.0, 1.0, 10000.0),
        160.0,
        |chart_width, _| draw(model, conf, options, chart_width),
    )
}

fn draw(
    model: &SankeyModel,
    conf: &Map,
    options: &Options,
    chart_width: f64,
) -> Result<Rendered, Error> {
    let p = crate::mermaid::PREFIX;
    let size = options.font_size.unwrap_or(16.0);
    let small = js::round(size * 0.875);
    let measure = options.measurer();
    let pad = options.padding.unwrap_or(8.0);

    let chart_height = conf.number_in("height", 400.0, 1.0, 10000.0);
    let node_width = conf
        .number_in("nodeWidth", 10.0, 1.0, 200.0)
        .min(chart_width);
    let show_values = conf.get("showValues") != Some(&Value::Bool(false));
    let prefix = text_option(conf, "prefix");
    let suffix = text_option(conf, "suffix");
    let outlined = conf.get("labelStyle").and_then(Value::as_str) == Some("outlined");
    let alignment = match conf.get("nodeAlignment").and_then(Value::as_str) {
        Some("left") => Alignment::Left,
        Some("right") => Alignment::Right,
        Some("center") => Alignment::Center,
        _ => Alignment::Justify,
    };
    let link_color = conf
        .get("linkColor")
        .and_then(Value::as_str)
        .unwrap_or("gradient");

    let layout = layout_sankey(
        model,
        &LayoutOptions {
            width: chart_width,
            height: chart_height,
            node_width,
            node_padding: conf.number_in("nodePadding", 12.0, 0.0, 200.0)
                + if show_values { VALUE_GAP } else { 0.0 },
            label_height: small * 1.15,
            alignment,
        },
    )?;
    let (nodes, links) = (&layout.nodes, &layout.links);

    let mut custom: HashMap<&str, String> = HashMap::new();
    if let Some(colors) = conf.section("nodeColors") {
        for (id, color) in colors.iter() {
            let style = fill_style(Some(color));
            if !style.is_empty() {
                custom.insert(id, style);
            }
        }
    }
    let node_style = |i: usize| -> &str {
        custom
            .get(model.nodes[i].id.as_str())
            .map_or("", String::as_str)
    };

    // Labels sit beside the bars, on the side that faces the middle of the
    // chart.
    let mut central = 0;
    let mut most = 0.0;
    for node in nodes {
        if node.value > most {
            most = node.value;
            central = node.layer;
        }
    }
    let mut min_x: f64 = 0.0;
    let mut max_x = chart_width;
    let mut min_y: f64 = 0.0;
    let mut max_y = chart_height;
    let mut labels_out = String::new();
    let mut nodes_out = String::new();
    for (i, node) in nodes.iter().enumerate() {
        let id = &model.nodes[i].id;
        let height = node.height.max(1.0);
        nodes_out.push_str(&format!(
            "<rect class=\"{p}-sankey-node\" data-id=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\"{}/>",
            esc_text(id),
            num(node.x),
            num(node.y),
            num(node_width),
            num(height),
            num(2f64.min(height / 2.0)),
            series_color(i as i64),
            node_style(i)
        ));

        let value = if show_values {
            format!("{prefix}{}{suffix}", js::num_str(rounded(node.value)))
        } else {
            String::new()
        };
        let after = if outlined {
            node.layer >= central
        } else {
            node.x < chart_width / 2.0
        };
        let text_width = measure.width(
            &if value.is_empty() {
                id.clone()
            } else {
                format!("{id} {value}")
            },
            small,
            0,
        );
        let x = if after {
            node.x + node_width + LABEL_GAP
        } else {
            node.x - LABEL_GAP
        };
        let y = node.y + node.height / 2.0;
        min_x = min_x.min(if after { x } else { x - text_width });
        max_x = max_x.max(if after { x + text_width } else { x });
        min_y = min_y.min(y - small * 0.75);
        max_y = max_y.max(y + small * 0.75);
        let position = format!(
            " x=\"{}\" y=\"{}\" text-anchor=\"{}\"",
            num(x),
            num(y + small * 0.35),
            if after { "start" } else { "end" }
        );
        let text = esc(id)
            + &if value.is_empty() {
                String::new()
            } else {
                format!(
                    " <tspan class=\"{p}-sankey-value\" fill=\"var(--_m)\">{}</tspan>",
                    esc(&value)
                )
            };
        labels_out.push_str(&format!(
            "<g class=\"{p}-label\" data-id=\"{}\">",
            esc_text(id)
        ));
        if outlined {
            labels_out.push_str(&format!(
                "<text{position} fill=\"none\" stroke=\"var(--_bg)\" stroke-width=\"3\" stroke-linejoin=\"round\">{}{}</text>",
                esc(id),
                if value.is_empty() {
                    String::new()
                } else {
                    format!(" {}", esc(&value))
                }
            ));
        }
        labels_out.push_str(&format!("<text{position}>{text}</text></g>"));
    }

    let fixed = if matches!(link_color, "gradient" | "source" | "target") {
        String::new()
    } else {
        fill_style(conf.get("linkColor"))
    };
    let mut links_out = String::new();
    for link in links {
        // Without definitions there are no gradients, so a gradient link
        // takes its source's colour.
        let end = if link_color == "target" {
            link.target
        } else {
            link.source
        };
        let half = link.thickness.max(1.0) / 2.0;
        let x0 = nodes[link.source].x + node_width;
        let x1 = nodes[link.target].x;
        let mid = num((x0 + x1) / 2.0);
        let from = esc_text(&model.nodes[link.source].id);
        let to = esc_text(&model.nodes[link.target].id);
        links_out.push_str(&format!(
            "<path class=\"{p}-sankey-link\" data-id=\"{from}-&gt;{to}\" d=\"M{},{}C{mid},{} {mid},{} {},{}L{},{}C{mid},{} {mid},{} {},{}Z\" fill=\"{}\"{}><title>{from} → {to}: {}{}{}</title></path>",
            num(x0),
            num(link.source_y - half),
            num(link.source_y - half),
            num(link.target_y - half),
            num(x1),
            num(link.target_y - half),
            num(x1),
            num(link.target_y + half),
            num(link.target_y + half),
            num(link.source_y + half),
            num(x0),
            num(link.source_y + half),
            series_color(end as i64),
            if fixed.is_empty() {
                node_style(end)
            } else {
                fixed.as_str()
            },
            esc_text(&prefix),
            js::num_str(rounded(link.value)),
            esc_text(&suffix)
        ));
    }

    let title = layout_label(
        model.title.as_deref(),
        false,
        &*measure,
        size,
        title_room(options),
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let inner = (max_x - min_x).max(title.width);
    let ox = pad - min_x + (inner - (max_x - min_x)) / 2.0;
    let oy = pad + title_height - min_y;
    let width = (inner + 2.0 * pad).ceil();
    let height = (max_y - min_y + title_height + 2.0 * pad).ceil();

    let content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
    ) + &format!(
        "<g transform=\"translate({},{})\"><g class=\"{p}-sankey-links\" fill-opacity=\"0.4\">{links_out}</g><g class=\"{p}-sankey-nodes\">{nodes_out}</g><g class=\"{p}-labels\" font-size=\"{}\">{labels_out}</g></g>",
        num(ox),
        num(oy),
        js::num_str(small)
    );
    let svg = svg_document(
        "sankey",
        width,
        height,
        size,
        options,
        Accessible {
            acc_title: model.acc_title.as_deref(),
            acc_descr: model.acc_descr.as_deref(),
        },
        &content,
    );

    Ok(Rendered {
        svg,
        width,
        height,
        links: Vec::new(),
    })
}
