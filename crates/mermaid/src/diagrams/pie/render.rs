//! Pele's `diagrams/pie/render.ts`.

use std::{f64::consts::PI, fmt::Write as _};

use super::model::PieModel;
use crate::{
    Config, Options, Rendered,
    config::Value,
    diagrams::common::fit_width::{fit_width, title_room},
    js,
    svg::{
        builder::{esc_text, label_svg, num},
        root::{Accessible, svg_document},
        theme::series_color,
    },
    text::{Label, Style, layout_label},
};

const RADIUS: f64 = 120.0;
const SWATCH: f64 = 12.0;
const ROW: f64 = 26.0;
const GAP: f64 = 32.0;
const OUTSIDE: f64 = 28.0;

fn option<'c>(config: &'c Config, key: &str) -> Option<&'c Value> {
    config.section("pie").and_then(|pie| pie.get(key))
}

fn point(angle: f64, radius: f64) -> String {
    format!(
        "{},{}",
        num(angle.sin() * radius),
        num(-angle.cos() * radius)
    )
}

fn slice(start: f64, end: f64, outer: f64, inner: f64) -> String {
    if end - start >= PI * 2.0 - 1e-9 {
        let ring = |r: f64, sweep: u8| {
            format!(
                "M0,{}A{},{} 0 1 {sweep} 0,{}A{},{} 0 1 {sweep} 0,{}Z",
                num(-r),
                num(r),
                num(r),
                num(r),
                num(r),
                num(r),
                num(-r)
            )
        };
        return ring(outer, 1)
            + &if inner > 0.0 {
                ring(inner, 0)
            } else {
                String::new()
            };
    }
    let large = if end - start > PI { 1 } else { 0 };
    if inner <= 0.0 {
        return format!(
            "M0,0L{}A{},{} 0 {large} 1 {}Z",
            point(start, outer),
            num(outer),
            num(outer),
            point(end, outer)
        );
    }
    format!(
        "M{}A{},{} 0 {large} 1 {}L{}A{},{} 0 {large} 0 {}Z",
        point(start, outer),
        num(outer),
        num(outer),
        point(end, outer),
        point(end, inner),
        num(inner),
        num(inner),
        point(start, inner)
    )
}

pub fn render_pie(model: &PieModel, config: &Config, options: &Options) -> Rendered {
    fit_width(options, RADIUS * 2.0, 140.0, |diameter, tight| {
        draw(model, config, options, diameter / 2.0, tight)
    })
}

/// `tight` is set when the chart does not fit the width it has with its
/// legend beside it.
fn draw(
    model: &PieModel,
    config: &Config,
    options: &Options,
    radius: f64,
    tight: bool,
) -> Rendered {
    let p = crate::PREFIX;
    let size = options.font_size.unwrap_or(16.0);
    let small = js::round(size * 0.875);
    let measure = options.measurer();
    let pad = options.padding.unwrap_or(8.0);

    let text_position = match option(config, "textPosition") {
        Some(Value::Number(n)) if *n >= 0.0 && *n <= 1.0 => *n,
        _ => 0.75,
    };
    let inner = match option(config, "donutHole") {
        Some(Value::Number(n)) if *n > 0.0 && *n <= 0.9 => n * radius,
        _ => 0.0,
    };
    let placed = match option(config, "legendPosition") {
        None | Some(Value::Null) => "right".to_string(),
        Some(value) => value.to_js_string(),
    };
    let legend_position = if tight && (placed == "left" || placed == "right") {
        "bottom"
    } else {
        placed.as_str()
    };

    let entries: Vec<(&String, f64)> = model.sections.iter().map(|(k, v)| (k, *v)).collect();
    let mut sum = 0.0;
    for (_, value) in &entries {
        sum += value;
    }

    let legend: Vec<Label> = entries
        .iter()
        .map(|(label, value)| {
            let text = if model.show_data {
                format!("{label} [{}]", js::num_str(*value))
            } else {
                label.to_string()
            };
            layout_label(Some(&text), false, &*measure, small, 4000.0, 0)
        })
        .collect();
    let mut legend_width: f64 = 0.0;
    for label in &legend {
        legend_width = legend_width.max(label.width);
    }
    legend_width += if !legend.is_empty() {
        SWATCH + 8.0
    } else {
        0.0
    };
    let legend_height = legend.len() as f64 * ROW;

    let title = layout_label(
        model.title.as_deref(),
        false,
        &*measure,
        size,
        title_room(options),
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 16.0
    } else {
        0.0
    };

    // A slice too thin for its percentage gets the label outside the rim,
    // which needs a margin.
    let shares: Vec<f64> = entries
        .iter()
        .map(|(_, value)| {
            let share = if sum > 0.0 { value / sum } else { 0.0 };
            if share * 100.0 >= 1.0 && js::to_fixed(share * 100.0, 0) != "0" {
                share
            } else {
                0.0
            }
        })
        .collect();
    let narrow: Vec<bool> = shares
        .iter()
        .map(|&share| {
            if share == 0.0 || share >= 0.5 {
                return false;
            }
            let chord = 2.0 * radius * text_position * (share * PI).sin();
            chord < measure.width(&format!("{}%", js::to_fixed(share * 100.0, 0)), small, 0) + 6.0
        })
        .collect();
    let margin = if narrow.contains(&true) { OUTSIDE } else { 0.0 };

    let side = if matches!(legend_position, "left" | "right" | "center") {
        legend_position
    } else {
        ""
    };
    let stacked = legend_position == "top" || legend_position == "bottom";
    let diameter = (radius + margin) * 2.0;
    let mut width = diameter;
    let mut height = diameter;
    if side == "left" || side == "right" {
        width += if legend_width > 0.0 {
            GAP + legend_width
        } else {
            0.0
        };
        height = height.max(legend_height);
    } else if stacked {
        width = width.max(legend_width);
        height += if legend_height > 0.0 {
            20.0 + legend_height
        } else {
            0.0
        };
    }
    width = width.max(title.width);

    let cx = pad
        + if side == "left" {
            legend_width + GAP + diameter / 2.0
        } else if side == "right" {
            diameter / 2.0
        } else {
            width / 2.0
        };
    let body_top = pad + title_height;
    let cy = body_top
        + if legend_position == "top" {
            legend_height + 20.0 + diameter / 2.0
        } else if stacked {
            diameter / 2.0
        } else {
            height / 2.0
        };

    let mut slices = String::new();
    let mut labels = String::new();
    let mut outside = String::new();
    let mut angle = 0.0;
    let rim = radius + OUTSIDE / 2.0;
    let mut placed_outside: Vec<(f64, f64, f64)> = Vec::new();
    for (index, (label, _)) in entries.iter().enumerate() {
        let share = shares[index];
        if share == 0.0 {
            continue;
        }
        let end = angle + share * PI * 2.0;
        let _ = write!(
            slices,
            "<path class=\"{p}-slice\" data-id=\"{}\" d=\"{}\" fill=\"{}\"/>",
            esc_text(label),
            slice(angle, end, radius, inner),
            series_color(index as i64)
        );
        let mid = (angle + end) / 2.0;
        let text = js::to_fixed(share * 100.0, 0) + "%";
        if narrow[index] {
            let width = measure.width(&text, small, 0);
            let clear = |x: f64, y: f64, placed: &[(f64, f64, f64)]| {
                placed.iter().all(|&(ox, oy, ow)| {
                    (x - ox).abs() >= (width + ow) / 2.0 + 2.0 || (y - oy).abs() >= small + 2.0
                })
            };
            let mut at = mid;
            let mut step = 0;
            while step < 360 && !clear(at.sin() * rim, -at.cos() * rim, &placed_outside) {
                at += 2.0 / rim;
                step += 1;
            }
            let x = at.sin() * rim;
            let y = -at.cos() * rim;
            placed_outside.push((x, y, width));
            let _ = write!(
                outside,
                "<text x=\"{}\" y=\"{}\">{text}</text>",
                num(x),
                num(y + small * 0.35)
            );
        } else {
            let r = radius * text_position;
            let _ = write!(
                labels,
                "<text x=\"{}\" y=\"{}\">{text}</text>",
                num(mid.sin() * r),
                num(-mid.cos() * r + small * 0.35)
            );
        }
        angle = end;
    }
    if slices.is_empty() {
        slices = format!(
            "<circle class=\"{p}-slice\" r=\"{}\" fill=\"none\" stroke=\"var(--_b)\"/>",
            js::num_str(radius)
        );
    }

    let mut legend_out = String::new();
    let (lx, ly) = if side == "right" {
        (
            cx + diameter / 2.0 + GAP,
            body_top + (height - legend_height) / 2.0,
        )
    } else if side == "left" {
        (pad, body_top + (height - legend_height) / 2.0)
    } else if side == "center" {
        (cx - legend_width / 2.0, cy - legend_height / 2.0)
    } else {
        (
            pad + (width - legend_width) / 2.0,
            if legend_position == "top" {
                body_top
            } else {
                body_top + diameter + 20.0
            },
        )
    };
    for (index, label) in legend.iter().enumerate() {
        let y = ly + index as f64 * ROW + ROW / 2.0;
        let _ = write!(
            legend_out,
            "<g class=\"{p}-legend-item\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\" fill=\"{}\"/>{}</g>",
            esc_text(entries[index].0),
            num(lx),
            num(y - SWATCH / 2.0),
            js::num_str(SWATCH),
            js::num_str(SWATCH),
            series_color(index as i64),
            label_svg(label, lx + SWATCH + 8.0 + label.width / 2.0, y, "")
        );
    }

    let total_width = (width + 2.0 * pad).ceil();
    let total_height = (height + title_height + 2.0 * pad).ceil();
    let small_str = js::num_str(small);
    let mut content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
    );
    let _ = write!(
        content,
        "<g class=\"{p}-slices\" transform=\"translate({},{})\" stroke=\"var(--_bg)\" stroke-width=\"1\" stroke-linejoin=\"round\">{slices}</g>",
        num(cx),
        num(cy)
    );
    if !labels.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-slice-labels\" transform=\"translate({},{})\" font-size=\"{small_str}\" text-anchor=\"middle\" fill=\"var(--_bg)\">{labels}</g>",
            num(cx),
            num(cy)
        );
    }
    if !outside.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-slice-labels\" transform=\"translate({},{})\" font-size=\"{small_str}\" text-anchor=\"middle\">{outside}</g>",
            num(cx),
            num(cy)
        );
    }
    if !legend_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-legend\" font-size=\"{small_str}\">{legend_out}</g>"
        );
    }

    Rendered {
        svg: svg_document(
            "pie",
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
