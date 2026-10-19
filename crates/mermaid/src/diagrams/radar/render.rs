//! Pele's `diagrams/radar/render.ts`.

use std::{f64::consts::PI, fmt::Write as _};

use super::model::RadarModel;
use crate::{
    Config, Options, Rendered,
    diagrams::common::fit_width::{fit_width, title_room},
    js,
    svg::{
        builder::{esc_text, label_svg, num, push_num},
        root::{Accessible, svg_document},
        theme::series_color,
    },
    text::{Label, Style, layout_label},
};

const SWATCH: f64 = 12.0;
const ROW: f64 = 26.0;
const GAP: f64 = 24.0;

#[derive(Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

fn option(config: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    config
        .section("radar")
        .map_or(fallback, |radar| radar.number_in(key, fallback, min, max))
}

pub fn relative_radius(value: f64, min: f64, max: f64, radius: f64) -> f64 {
    let clipped = js::min(js::max(value, min), max);
    (radius * (clipped - min)) / (max - min)
}

/// A closed Catmull-Rom spline through the points, as cubic Bézier segments.
pub fn closed_round_curve(points: &[Point], tension: f64) -> String {
    let n = points.len();
    let mut d = format!("M{},{}", num(points[0].x), num(points[0].y));
    for i in 0..n {
        let p0 = points[(i + n - 1) % n];
        let p1 = points[i];
        let p2 = points[(i + 1) % n];
        let p3 = points[(i + 2) % n];
        let _ = write!(
            d,
            " C{},{} {},{} {},{}",
            num(p1.x + (p2.x - p0.x) * tension),
            num(p1.y + (p2.y - p0.y) * tension),
            num(p2.x - (p3.x - p1.x) * tension),
            num(p2.y - (p3.y - p1.y) * tension),
            num(p2.x),
            num(p2.y)
        );
    }
    d + " Z"
}

fn polygon(points: &[Point]) -> String {
    let mut out = String::new();
    for p in points {
        if !out.is_empty() {
            out.push(' ');
        }
        push_num(&mut out, p.x);
        out.push(',');
        push_num(&mut out, p.y);
    }
    out
}

pub fn render_radar(model: &RadarModel, config: &Config, options: &Options) -> Rendered {
    let natural = option(config, "width", 320.0, 1.0, 4000.0);
    fit_width(options, natural, 160.0, |width, tight| {
        draw(
            model,
            config,
            options,
            width,
            (option(config, "height", 320.0, 1.0, 4000.0) * width) / natural,
            tight,
        )
    })
}

/// `tight` is set when the chart does not fit the width it has with its
/// legend beside it.
fn draw(
    model: &RadarModel,
    config: &Config,
    options: &Options,
    width: f64,
    height: f64,
    tight: bool,
) -> Rendered {
    let p = crate::PREFIX;
    let size = options.font_size.unwrap_or(16.0);
    let small = js::round(size * 0.875);
    let measure = options.measurer();
    let pad = options.padding.unwrap_or(8.0);

    let axis_scale = option(config, "axisScaleFactor", 1.0, 0.0, 10.0);
    let label_factor = option(config, "axisLabelFactor", 1.05, 0.0, 10.0);
    let tension = option(config, "curveTension", 0.17, 0.0, 1.0);
    let radius = width.min(height) / 2.0;

    let axes = &model.axes;
    let curves = &model.curves;
    let ticks = model.options.ticks;
    let graticule = model.options.graticule.as_str();
    let min = model.options.min;
    let count = axes.len();
    let unit: Vec<Point> = (0..count)
        .map(|i| {
            let angle = (2.0 * PI * i as f64) / count as f64 - PI / 2.0;
            Point {
                x: angle.cos(),
                y: angle.sin(),
            }
        })
        .collect();

    let max = match model.options.max {
        Some(max) => max,
        None => {
            let mut max = f64::NEG_INFINITY;
            for curve in curves {
                for &value in &curve.entries {
                    if value > max {
                        max = value;
                    }
                }
            }
            max
        }
    };

    // The chart is drawn around the origin. Margins reserve space; labels and
    // the legend widen it.
    let mut x0 = -width / 2.0 - option(config, "marginLeft", 16.0, 0.0, 2000.0);
    let mut x1 = width / 2.0 + option(config, "marginRight", 16.0, 0.0, 2000.0);
    let mut y0 = -height / 2.0 - option(config, "marginTop", 16.0, 0.0, 2000.0);
    let mut y1 = height / 2.0 + option(config, "marginBottom", 16.0, 0.0, 2000.0);

    let mut grid = String::new();
    let mut i = 0.0;
    while i < ticks {
        let r = radius.min((radius * (i + 1.0)) / ticks);
        if graticule == "circle" {
            let _ = write!(grid, "<circle r=\"{}\"/>", num(r));
        } else {
            let points: Vec<Point> = unit
                .iter()
                .map(|u| Point {
                    x: u.x * r,
                    y: u.y * r,
                })
                .collect();
            let _ = write!(grid, "<polygon points=\"{}\"/>", polygon(&points));
        }
        i += 1.0;
    }

    let mut spokes = String::new();
    let mut labels = String::new();
    for (i, u) in unit.iter().enumerate() {
        let reach = radius * axis_scale;
        let _ = write!(spokes, "M0,0L{},{}", num(u.x * reach), num(u.y * reach));
        let label = layout_label(Some(&axes[i].label), false, &*measure, small, 4000.0, 0);
        let at = radius * label_factor + 6.0;
        // Each label hangs off its axis on the side away from the center.
        let cx = u.x * at
            + if u.x > 0.01 {
                label.width / 2.0
            } else if u.x < -0.01 {
                -label.width / 2.0
            } else {
                0.0
            };
        let cy = u.y * at
            + if u.y > 0.01 {
                label.height / 2.0
            } else if u.y < -0.01 {
                -label.height / 2.0
            } else {
                0.0
            };
        let _ = write!(
            labels,
            "<g class=\"{p}-axis-label\" data-id=\"{}\">{}</g>",
            esc_text(&axes[i].name),
            label_svg(&label, cx, cy, "")
        );
        x0 = x0.min(cx - label.width / 2.0);
        x1 = x1.max(cx + label.width / 2.0);
        y0 = y0.min(cy - label.height / 2.0);
        y1 = y1.max(cy + label.height / 2.0);
    }

    let mut shapes = String::new();
    for (index, curve) in curves.iter().enumerate() {
        // Mermaid leaves out a curve that does not have a value for every
        // axis.
        if curve.entries.len() != count {
            continue;
        }
        let points: Vec<Point> = curve
            .entries
            .iter()
            .enumerate()
            .map(|(i, &value)| {
                let r = relative_radius(value, min, max, radius);
                let safe = if r.is_finite() { r } else { 0.0 };
                Point {
                    x: unit[i].x * safe,
                    y: unit[i].y * safe,
                }
            })
            .collect();
        let color = series_color(index as i64);
        let attrs = format!(
            " class=\"{p}-curve\" data-id=\"{}\" fill=\"{color}\" stroke=\"{color}\"",
            esc_text(&curve.name)
        );
        if graticule == "circle" {
            let _ = write!(
                shapes,
                "<path{attrs} d=\"{}\"/>",
                closed_round_curve(&points, tension)
            );
        } else {
            let _ = write!(shapes, "<polygon{attrs} points=\"{}\"/>", polygon(&points));
        }
    }

    let legend: Vec<Label> = if model.options.show_legend {
        curves
            .iter()
            .map(|curve| layout_label(Some(&curve.label), false, &*measure, small, 4000.0, 0))
            .collect()
    } else {
        Vec::new()
    };
    let mut legend_width: f64 = 0.0;
    for label in &legend {
        legend_width = legend_width.max(label.width);
    }
    let legend_height = legend.len() as f64 * ROW;
    let legend_x = if tight {
        -(SWATCH + 8.0 + legend_width) / 2.0
    } else {
        x1 + GAP
    };
    let legend_top = if tight {
        y1 + GAP / 2.0
    } else {
        -legend_height / 2.0
    };
    if !legend.is_empty() && tight {
        x0 = x0.min(legend_x);
        x1 = x1.max(-legend_x);
        y1 = legend_top + legend_height;
    } else if !legend.is_empty() {
        x1 = legend_x + SWATCH + 8.0 + legend_width;
        y0 = y0.min(-legend_height / 2.0);
        y1 = y1.max(legend_height / 2.0);
    }
    let mut legend_out = String::new();
    for (index, label) in legend.iter().enumerate() {
        let y = legend_top + index as f64 * ROW + ROW / 2.0;
        let color = series_color(index as i64);
        let _ = write!(
            legend_out,
            "<g class=\"{p}-legend-item\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\" fill=\"{color}\" fill-opacity=\"0.4\" stroke=\"{color}\"/>{}</g>",
            esc_text(&curves[index].name),
            num(legend_x),
            num(y - SWATCH / 2.0),
            js::num_str(SWATCH),
            js::num_str(SWATCH),
            label_svg(label, legend_x + SWATCH + 8.0 + label.width / 2.0, y, "")
        );
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
        title.height + 8.0
    } else {
        0.0
    };
    let body_width = (x1 - x0).max(title.width);
    let total_width = (body_width + 2.0 * pad).ceil();
    let total_height = (y1 - y0 + title_height + 2.0 * pad).ceil();
    let origin = format!(
        "translate({},{})",
        num(pad + (body_width - (x1 - x0)) / 2.0 - x0),
        num(pad + title_height - y0)
    );

    let small = js::num_str(small);
    let mut content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
    );
    let _ = write!(content, "<g transform=\"{origin}\">");
    if !grid.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-graticule\" fill=\"none\" stroke=\"var(--_a)\">{grid}</g>"
        );
    }
    if !spokes.is_empty() {
        let _ = write!(
            content,
            "<path class=\"{p}-axes\" d=\"{spokes}\" fill=\"none\" stroke=\"var(--_b)\"/>"
        );
    }
    if !shapes.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-curves\" fill-opacity=\"0.2\" stroke-width=\"1.5\" stroke-linejoin=\"round\">{shapes}</g>"
        );
    }
    if !labels.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-axis-labels\" font-size=\"{small}\">{labels}</g>"
        );
    }
    if !legend_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-legend\" font-size=\"{small}\">{legend_out}</g>"
        );
    }
    content.push_str("</g>");

    Rendered {
        svg: svg_document(
            "radar",
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
