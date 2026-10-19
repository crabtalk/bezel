//! Pele's `diagrams/radar/render.ts`.

use crate::config::capped;
use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};
use std::{
    f64::consts::PI,
    fmt::{self, Write as _},
};

use super::model::RadarModel;
use crate::{Ctx, Host};
use crate::{
    Rendered,
    diagrams::common::fit_width::{fit_width, title_room},
    js,
    svg::{
        builder::{EscText, LabelSvg, Point as Xy, fragment, num},
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

pub fn relative_radius(value: f64, min: f64, max: f64, radius: f64) -> f64 {
    let clipped = js::min(js::max(value, min), max);
    (radius * (clipped - min)) / (max - min)
}

/// A closed Catmull-Rom spline through the points, as cubic Bézier segments.
pub fn closed_round_curve(points: &[Point], tension: f64) -> impl fmt::Display + '_ {
    fragment(move |f| {
        let n = points.len();
        write!(f, "M{}", Xy(points[0].x, points[0].y))?;
        for i in 0..n {
            let p0 = points[(i + n - 1) % n];
            let p1 = points[i];
            let p2 = points[(i + 1) % n];
            let p3 = points[(i + 2) % n];
            write!(
                f,
                " C{},{} {},{} {},{}",
                num(p1.x + (p2.x - p0.x) * tension),
                num(p1.y + (p2.y - p0.y) * tension),
                num(p2.x - (p3.x - p1.x) * tension),
                num(p2.y - (p3.y - p1.y) * tension),
                num(p2.x),
                num(p2.y)
            )?;
        }
        f.write_str(" Z")
    })
}

fn polygon(points: impl Iterator<Item = Point> + Clone) -> impl fmt::Display {
    fragment(move |f| {
        for (i, p) in points.clone().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{}", Xy(p.x, p.y))?;
        }
        Ok(())
    })
}

/// Mermaid's `radar` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RadarConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub axis_label_factor: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub axis_scale_factor: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub curve_tension: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub height: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub margin_bottom: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub margin_left: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub margin_right: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub margin_top: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub width: Option<f64>,
}

pub fn render_radar(
    model: &RadarModel,
    config: &RadarConfig,
    ctx: &Ctx<'_, impl Host>,
) -> Rendered {
    let natural = capped(config.width, 320.0, 1.0, 4000.0);
    fit_width(ctx, natural, 160.0, |width, tight| {
        draw(
            model,
            config,
            ctx,
            width,
            (capped(config.height, 320.0, 1.0, 4000.0) * width) / natural,
            tight,
        )
    })
}

/// `tight` is set when the chart does not fit the width it has with its
/// legend beside it.
fn draw(
    model: &RadarModel,
    config: &RadarConfig,
    ctx: &Ctx<'_, impl Host>,
    width: f64,
    height: f64,
    tight: bool,
) -> Rendered {
    let p = crate::PREFIX;
    let size = ctx.options.font_size;
    let small = js::round(size * 0.875);
    let measure = ctx.host;
    let pad = ctx.options.padding.unwrap_or(8.0);

    let axis_scale = capped(config.axis_scale_factor, 1.0, 0.0, 10.0);
    let label_factor = capped(config.axis_label_factor, 1.05, 0.0, 10.0);
    let tension = capped(config.curve_tension, 0.17, 0.0, 1.0);
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
    let mut x0 = -width / 2.0 - capped(config.margin_left, 16.0, 0.0, 2000.0);
    let mut x1 = width / 2.0 + capped(config.margin_right, 16.0, 0.0, 2000.0);
    let mut y0 = -height / 2.0 - capped(config.margin_top, 16.0, 0.0, 2000.0);
    let mut y1 = height / 2.0 + capped(config.margin_bottom, 16.0, 0.0, 2000.0);

    let reach = radius * axis_scale;
    let at = radius * label_factor + 6.0;
    let mut axis_labels = Vec::with_capacity(count);
    for (axis, u) in axes.iter().zip(&unit) {
        let label = layout_label(Some(&axis.label), false, measure, small, 4000.0, 0);
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
        x0 = x0.min(cx - label.width / 2.0);
        x1 = x1.max(cx + label.width / 2.0);
        y0 = y0.min(cy - label.height / 2.0);
        y1 = y1.max(cy + label.height / 2.0);
        axis_labels.push((label, cx, cy));
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
    let title = layout_label(
        model.title.as_deref(),
        false,
        measure,
        size,
        title_room(ctx),
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

    let small = js::Number(small);
    let mut content = String::new();
    let _ = write!(
        content,
        "{}<g transform=\"translate({},{})\">",
        LabelSvg(
            &title,
            total_width / 2.0,
            pad + title.height / 2.0,
            crate::svg::builder::TITLE,
        ),
        num(pad + (body_width - (x1 - x0)) / 2.0 - x0),
        num(pad + title_height - y0)
    );
    if ticks > 0.0 {
        let _ = write!(
            content,
            "<g class=\"{p}-graticule\" fill=\"none\" stroke=\"var(--_a)\">"
        );
        let mut i = 0.0;
        while i < ticks {
            let r = radius.min((radius * (i + 1.0)) / ticks);
            if graticule == "circle" {
                let _ = write!(content, "<circle r=\"{}\"/>", num(r));
            } else {
                let _ = write!(
                    content,
                    "<polygon points=\"{}\"/>",
                    polygon(unit.iter().map(|u| Point {
                        x: u.x * r,
                        y: u.y * r,
                    }))
                );
            }
            i += 1.0;
        }
        content.push_str("</g>");
    }
    if count > 0 {
        let _ = write!(content, "<path class=\"{p}-axes\" d=\"");
        for u in &unit {
            let _ = write!(content, "M0,0L{}", Xy(u.x * reach, u.y * reach));
        }
        content.push_str("\" fill=\"none\" stroke=\"var(--_b)\"/>");
    }
    let mut drawn = curves
        .iter()
        .enumerate()
        // Mermaid leaves out a curve that does not have a value for every
        // axis.
        .filter(|(_, curve)| curve.entries.len() == count)
        .peekable();
    if drawn.peek().is_some() {
        let _ = write!(
            content,
            "<g class=\"{p}-curves\" fill-opacity=\"0.2\" stroke-width=\"1.5\" stroke-linejoin=\"round\">"
        );
        for (index, curve) in drawn {
            let points = curve.entries.iter().zip(&unit).map(|(&value, u)| {
                let r = relative_radius(value, min, max, radius);
                let safe = if r.is_finite() { r } else { 0.0 };
                Point {
                    x: u.x * safe,
                    y: u.y * safe,
                }
            });
            let color = series_color(index as i64);
            let _ = write!(
                content,
                "<{} class=\"{p}-curve\" data-id=\"{}\" fill=\"{color}\" stroke=\"{color}\"",
                if graticule == "circle" {
                    "path"
                } else {
                    "polygon"
                },
                EscText(&curve.name)
            );
            if graticule == "circle" {
                let points: Vec<Point> = points.collect();
                let _ = write!(content, " d=\"{}\"/>", closed_round_curve(&points, tension));
            } else {
                let _ = write!(content, " points=\"{}\"/>", polygon(points));
            }
        }
        content.push_str("</g>");
    }
    if count > 0 {
        let _ = write!(
            content,
            "<g class=\"{p}-axis-labels\" font-size=\"{small}\">"
        );
        for (axis, (label, cx, cy)) in axes.iter().zip(&axis_labels) {
            let _ = write!(
                content,
                "<g class=\"{p}-axis-label\" data-id=\"{}\">{}</g>",
                EscText(&axis.name),
                LabelSvg(label, *cx, *cy, "")
            );
        }
        content.push_str("</g>");
    }
    if !legend.is_empty() {
        let _ = write!(content, "<g class=\"{p}-legend\" font-size=\"{small}\">");
        for (index, label) in legend.iter().enumerate() {
            let y = legend_top + index as f64 * ROW + ROW / 2.0;
            let color = series_color(index as i64);
            let _ = write!(
                content,
                "<g class=\"{p}-legend-item\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\" fill=\"{color}\" fill-opacity=\"0.4\" stroke=\"{color}\"/>{}</g>",
                EscText(&curves[index].name),
                num(legend_x),
                num(y - SWATCH / 2.0),
                js::Number(SWATCH),
                js::Number(SWATCH),
                LabelSvg(label, legend_x + SWATCH + 8.0 + label.width / 2.0, y, "")
            );
        }
        content.push_str("</g>");
    }
    content.push_str("</g>");

    Rendered {
        svg: svg_document(
            "radar",
            total_width,
            total_height,
            size,
            ctx,
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
