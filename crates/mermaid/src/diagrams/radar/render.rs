//! Pele's `diagrams/radar/render.ts`.

use crate::config::capped;
use js::{curve::Curve, math, number::Number, path::Segment};
use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};
use std::{
    f64::consts::PI,
    fmt::{self, Write as _},
};

use super::model::{Graticule, RadarModel};
use crate::{
    Config, Ctx, Error, Host, Rendered,
    diagrams::{
        Render,
        common::legend::{ROW, SWATCH},
    },
    geom::Point,
    svg::{
        builder::{EscText, fragment, num},
        root::svg_document,
        theme::series_color,
    },
    text::{Label, Style},
};

const GAP: f64 = 24.0;

fn relative_radius(value: f64, min: f64, max: f64, radius: f64) -> f64 {
    let clipped = math::min(math::max(value, min), max);
    (radius * (clipped - min)) / (max - min)
}

/// d3's closed cardinal curve through the points, from the first: each
/// control point is `tension` times the span of its end's neighbours away.
fn closed_round_curve(points: &[Point], tension: f64) -> impl fmt::Display + '_ {
    fragment(move |f| {
        // d3 starts a closed curve at its second point.
        let run: Vec<[f64; 2]> = points
            .iter()
            .cycle()
            .skip(points.len().saturating_sub(1))
            .take(points.len())
            .map(|p| [p.x, p.y])
            .collect();
        let at = |[x, y]: [f64; 2]| Point::new(x, y);
        let mut segments = Curve::CardinalClosed(1.0 - 6.0 * tension).line(&run);
        // d3 leaves a closed curve of three or more points unclosed.
        if segments.last() != Some(&Segment::Close) {
            segments.push(Segment::Close);
        }
        for (i, segment) in segments.into_iter().enumerate() {
            let gap = if i > 0 { " " } else { "" };
            match segment {
                Segment::Move(p) => write!(f, "{gap}M{}", at(p))?,
                Segment::Line(p) => write!(f, "{gap}L{}", at(p))?,
                Segment::Cubic(a, b, p) => write!(
                    f,
                    "{gap}C{},{} {},{} {},{}",
                    num(a[0]),
                    num(a[1]),
                    num(b[0]),
                    num(b[1]),
                    num(p[0]),
                    num(p[1])
                )?,
                Segment::Arc {
                    radius,
                    large,
                    sweep,
                    to,
                } => write!(
                    f,
                    "{gap}A{},{} 0 {} {} {},{}",
                    num(radius),
                    num(radius),
                    u8::from(large),
                    u8::from(sweep),
                    num(to[0]),
                    num(to[1])
                )?,
                Segment::Close => write!(f, "{gap}Z")?,
            }
        }
        Ok(())
    })
}

fn polygon(points: impl Iterator<Item = Point> + Clone) -> impl fmt::Display {
    fragment(move |f| {
        for (i, p) in points.clone().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{}", p)?;
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

impl Render for RadarModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        let config = &config.radar;
        let natural = capped(config.width, 320.0, 1.0, 4000.0);
        Ok(ctx.fit_width(natural, 160.0, |width, tight| {
            self.draw(
                config,
                ctx,
                width,
                (capped(config.height, 320.0, 1.0, 4000.0) * width) / natural,
                tight,
            )
        }))
    }
}

impl RadarModel {
    /// `tight` is set when the chart does not fit the width it has with its
    /// legend beside it.
    fn draw(
        &self,
        config: &RadarConfig,
        ctx: &Ctx<'_, impl Host>,
        width: f64,
        height: f64,
        tight: bool,
    ) -> Rendered {
        let p = crate::PREFIX;
        let size = ctx.options.font_size;
        let small = math::round(size * 0.875);
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);

        let axis_scale = capped(config.axis_scale_factor, 1.0, 0.0, 10.0);
        let label_factor = capped(config.axis_label_factor, 1.05, 0.0, 10.0);
        let tension = capped(config.curve_tension, 0.17, 0.0, 1.0);
        let radius = width.min(height) / 2.0;

        let axes = &self.axes;
        let curves = &self.curves;
        let ticks = self.options.ticks;
        let graticule = self.options.graticule;
        let bottom = self.options.min;
        let count = axes.len();
        let unit: Vec<Point> = (0..count)
            .map(|i| {
                let angle = (2.0 * PI * i as f64) / count as f64 - PI / 2.0;
                Point::new(angle.cos(), angle.sin())
            })
            .collect();

        let top = match self.options.max {
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
        let mut min = Point::new(
            -width / 2.0 - capped(config.margin_left, 16.0, 0.0, 2000.0),
            -height / 2.0 - capped(config.margin_top, 16.0, 0.0, 2000.0),
        );
        let mut max = Point::new(
            width / 2.0 + capped(config.margin_right, 16.0, 0.0, 2000.0),
            height / 2.0 + capped(config.margin_bottom, 16.0, 0.0, 2000.0),
        );

        let reach = radius * axis_scale;
        let at = radius * label_factor + 6.0;
        let mut axis_labels = Vec::with_capacity(count);
        for (axis, u) in axes.iter().zip(&unit) {
            let label = Label::layout(Some(&axis.label), false, measure, small, 4000.0, 0);
            // Each label hangs off its axis on the side away from the center.
            let hang = |along: f64, half: f64| {
                if along > 0.01 {
                    half
                } else if along < -0.01 {
                    -half
                } else {
                    0.0
                }
            };
            let center = Point::new(
                u.x * at + hang(u.x, label.width / 2.0),
                u.y * at + hang(u.y, label.height / 2.0),
            );
            min.x = min.x.min(center.x - label.width / 2.0);
            max.x = max.x.max(center.x + label.width / 2.0);
            min.y = min.y.min(center.y - label.height / 2.0);
            max.y = max.y.max(center.y + label.height / 2.0);
            axis_labels.push((label, center));
        }

        let legend: Vec<Label> = if self.options.show_legend {
            curves
                .iter()
                .map(|curve| Label::layout(Some(&curve.label), false, &*measure, small, 4000.0, 0))
                .collect()
        } else {
            Vec::new()
        };
        let mut legend_width: f64 = 0.0;
        for label in &legend {
            legend_width = legend_width.max(label.width);
        }
        let legend_height = legend.len() as f64 * ROW;
        let legend_at = if tight {
            Point::new(-(SWATCH + 8.0 + legend_width) / 2.0, max.y + GAP / 2.0)
        } else {
            Point::new(max.x + GAP, -legend_height / 2.0)
        };
        if !legend.is_empty() && tight {
            min.x = min.x.min(legend_at.x);
            max.x = max.x.max(-legend_at.x);
            max.y = legend_at.y + legend_height;
        } else if !legend.is_empty() {
            max.x = legend_at.x + SWATCH + 8.0 + legend_width;
            min.y = min.y.min(-legend_height / 2.0);
            max.y = max.y.max(legend_height / 2.0);
        }
        let title = Label::layout(
            self.titles.title.as_deref(),
            false,
            measure,
            size,
            ctx.title_room(),
            Style::BOLD,
        );
        let title_height = title.spaced_height(8.0);
        let body_width = (max.x - min.x).max(title.width);
        let total_width = (body_width + 2.0 * pad).ceil();
        let total_height = (max.y - min.y + title_height + 2.0 * pad).ceil();

        let small = Number(small);
        let mut content = String::new();
        let _ = write!(
            content,
            "{}<g transform=\"translate({},{})\">",
            title.svg(
                Point::new(total_width / 2.0, pad + title.height / 2.0),
                crate::svg::builder::TITLE
            ),
            num(pad + (body_width - (max.x - min.x)) / 2.0 - min.x),
            num(pad + title_height - min.y)
        );
        if ticks > 0.0 {
            let _ = write!(
                content,
                "<g class=\"{p}-graticule\" fill=\"none\" stroke=\"var(--_a)\">"
            );
            let mut i = 0.0;
            while i < ticks {
                let r = radius.min((radius * (i + 1.0)) / ticks);
                let _ = match graticule {
                    Graticule::Circle => write!(content, "<circle r=\"{}\"/>", num(r)),
                    Graticule::Polygon => write!(
                        content,
                        "<polygon points=\"{}\"/>",
                        polygon(unit.iter().map(|&u| u * r))
                    ),
                };
                i += 1.0;
            }
            content.push_str("</g>");
        }
        if count > 0 {
            let _ = write!(content, "<path class=\"{p}-axes\" d=\"");
            for u in &unit {
                let _ = write!(content, "M0,0L{}", *u * reach);
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
                    let r = relative_radius(value, bottom, top, radius);
                    let safe = if r.is_finite() { r } else { 0.0 };
                    *u * safe
                });
                let color = series_color(index as i64);
                let _ = write!(
                    content,
                    "<{} class=\"{p}-curve\" data-id=\"{}\" fill=\"{color}\" stroke=\"{color}\"",
                    match graticule {
                        Graticule::Circle => "path",
                        Graticule::Polygon => "polygon",
                    },
                    EscText(&curve.name)
                );
                let _ = match graticule {
                    Graticule::Circle => {
                        let points: Vec<Point> = points.collect();
                        write!(content, " d=\"{}\"/>", closed_round_curve(&points, tension))
                    }
                    Graticule::Polygon => write!(content, " points=\"{}\"/>", polygon(points)),
                };
            }
            content.push_str("</g>");
        }
        if count > 0 {
            let _ = write!(
                content,
                "<g class=\"{p}-axis-labels\" font-size=\"{small}\">"
            );
            for (axis, (label, center)) in axes.iter().zip(&axis_labels) {
                let _ = write!(
                    content,
                    "<g class=\"{p}-axis-label\" data-id=\"{}\">{}</g>",
                    EscText(&axis.name),
                    label.svg(*center, "")
                );
            }
            content.push_str("</g>");
        }
        if !legend.is_empty() {
            let _ = write!(content, "<g class=\"{p}-legend\" font-size=\"{small}\">");
            for (index, label) in legend.iter().enumerate() {
                let y = legend_at.y + index as f64 * ROW + ROW / 2.0;
                let color = series_color(index as i64);
                let _ = write!(
                    content,
                    "<g class=\"{p}-legend-item\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\" fill=\"{color}\" fill-opacity=\"0.4\" stroke=\"{color}\"/>{}</g>",
                    EscText(&curves[index].name),
                    num(legend_at.x),
                    num(y - SWATCH / 2.0),
                    Number(SWATCH),
                    Number(SWATCH),
                    label.svg(
                        Point::new(legend_at.x + SWATCH + 8.0 + label.width / 2.0, y),
                        ""
                    )
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
                self.titles.accessible(),
                &content,
            ),
            width: total_width,
            height: total_height,
            links: Vec::new(),
        }
    }
}
