//! Pele's `diagrams/pie/render.ts`.

use crate::geom::Point;
use crate::js::JsValue;
use crate::svg::builder::{EscText, fragment};
use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};
use std::{
    f64::consts::PI,
    fmt::{self, Write as _},
};

use super::model::PieModel;
use crate::diagrams::common::legend::{ROW, SWATCH};
use crate::{Config, Ctx, Error, Host};
use crate::{
    Rendered,
    config::Value,
    diagrams::Render,
    js,
    svg::{builder::num, root::svg_document, theme::series_color},
    text::{Label, Style},
};

const RADIUS: f64 = 120.0;
const GAP: f64 = 32.0;
const OUTSIDE: f64 = 28.0;

/// Mermaid's `pie` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PieConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub text_position: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub donut_hole: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub legend_position: Option<Value>,
}

/// Where a legend goes. Without one, the legend goes below the chart and
/// the chart makes no room for it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LegendPosition {
    Left,
    Right,
    Center,
    Top,
    Bottom,
}

impl LegendPosition {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "left" => Self::Left,
            "right" => Self::Right,
            "center" => Self::Center,
            "top" => Self::Top,
            "bottom" => Self::Bottom,
            _ => return None,
        })
    }

    /// Left or right of the chart.
    fn is_beside(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    /// Above or below the chart.
    fn is_stacked(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
}

/// The point at `angle` clockwise from twelve o'clock, `radius` from the
/// centre.
fn point(angle: f64, radius: f64) -> Point {
    Point::new(angle.sin() * radius, -angle.cos() * radius)
}

fn slice(start: f64, end: f64, outer: f64, inner: f64) -> impl fmt::Display {
    fragment(move |f| {
        if end - start >= PI * 2.0 - 1e-9 {
            let ring = |f: &mut fmt::Formatter<'_>, r: f64, sweep: u8| {
                write!(
                    f,
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
            ring(f, outer, 1)?;
            if inner > 0.0 {
                ring(f, inner, 0)?;
            }
            return Ok(());
        }
        let large = if end - start > PI { 1 } else { 0 };
        if inner <= 0.0 {
            return write!(
                f,
                "M0,0L{}A{},{} 0 {large} 1 {}Z",
                point(start, outer),
                num(outer),
                num(outer),
                point(end, outer)
            );
        }
        write!(
            f,
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
    })
}

impl Render for PieModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(ctx.fit_width(RADIUS * 2.0, 140.0, |diameter, tight| {
            self.draw(&config.pie, ctx, diameter / 2.0, tight)
        }))
    }
}

impl PieModel {
    /// `tight` is set when the chart does not fit the width it has with its
    /// legend beside it.
    fn draw(
        &self,
        config: &PieConfig,
        ctx: &Ctx<'_, impl Host>,
        radius: f64,
        tight: bool,
    ) -> Rendered {
        let p = crate::PREFIX;
        let size = ctx.options.font_size;
        let small = js::round(size * 0.875);
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);

        let text_position = match config.text_position {
            Some(n) if (0.0..=1.0).contains(&n) => n,
            _ => 0.75,
        };
        let inner = match config.donut_hole {
            Some(n) if n > 0.0 && n <= 0.9 => n * radius,
            _ => 0.0,
        };
        let placed = match &config.legend_position {
            None | Some(Value::Null) => Some(LegendPosition::Right),
            Some(value) => LegendPosition::from_name(&value.to_js_string()),
        };
        let legend_position = match placed {
            Some(position) if tight && position.is_beside() => Some(LegendPosition::Bottom),
            placed => placed,
        };

        let entries: Vec<(&String, f64)> = self.sections.iter().map(|(k, v)| (k, *v)).collect();
        let mut sum = 0.0;
        for (_, value) in &entries {
            sum += value;
        }

        let legend: Vec<Label> = entries
            .iter()
            .map(|(label, value)| {
                let text = if self.show_data {
                    format!("{label} [{}]", js::Number(*value))
                } else {
                    label.to_string()
                };
                Label::layout(Some(&text), false, &*measure, small, 4000.0, 0)
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

        let title = Label::layout(
            self.titles.title.as_deref(),
            false,
            measure,
            size,
            ctx.title_room(),
            Style::BOLD,
        );
        let title_height = title.spaced_height(16.0);

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
                chord
                    < measure.width(&format!("{}%", js::to_fixed(share * 100.0, 0)), small, 0) + 6.0
            })
            .collect();
        let margin = if narrow.contains(&true) { OUTSIDE } else { 0.0 };

        let stacked = legend_position.is_some_and(LegendPosition::is_stacked);
        let diameter = (radius + margin) * 2.0;
        let mut width = diameter;
        let mut height = diameter;
        if legend_position.is_some_and(LegendPosition::is_beside) {
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

        let body_top = pad + title_height;
        let center = Point::new(
            pad + match legend_position {
                Some(LegendPosition::Left) => legend_width + GAP + diameter / 2.0,
                Some(LegendPosition::Right) => diameter / 2.0,
                _ => width / 2.0,
            },
            body_top
                + match legend_position {
                    Some(LegendPosition::Top) => legend_height + 20.0 + diameter / 2.0,
                    _ if stacked => diameter / 2.0,
                    _ => height / 2.0,
                },
        );

        let total_width = (width + 2.0 * pad).ceil();
        let total_height = (height + title_height + 2.0 * pad).ceil();
        let mut content = String::new();
        let _ = write!(
            content,
            "{}<g class=\"{p}-slices\" transform=\"translate({center})\" stroke=\"var(--_bg)\" stroke-width=\"1\" stroke-linejoin=\"round\">",
            title.svg(
                Point::new(total_width / 2.0, pad + title.height / 2.0),
                crate::svg::builder::TITLE
            ),
        );
        let arcs = || {
            shares
                .iter()
                .enumerate()
                .scan(0.0, |angle, (index, &share)| {
                    let start = *angle;
                    if share != 0.0 {
                        *angle = start + share * PI * 2.0;
                    }
                    Some((index, share, start, *angle))
                })
                .filter(|&(_, share, _, _)| share != 0.0)
        };
        let mut drawn = false;
        for (index, _, start, end) in arcs() {
            drawn = true;
            let _ = write!(
                content,
                "<path class=\"{p}-slice\" data-id=\"{}\" d=\"{}\" fill=\"{}\"/>",
                EscText(entries[index].0),
                slice(start, end, radius, inner),
                series_color(index as i64)
            );
        }
        if !drawn {
            let _ = write!(
                content,
                "<circle class=\"{p}-slice\" r=\"{}\" fill=\"none\" stroke=\"var(--_b)\"/>",
                js::Number(radius)
            );
        }
        content.push_str("</g>");

        let percent = |share: f64| js::to_fixed(share * 100.0, 0) + "%";
        let mut inside = arcs().filter(|&(index, ..)| !narrow[index]).peekable();
        if inside.peek().is_some() {
            let _ = write!(
                content,
                "<g class=\"{p}-slice-labels\" transform=\"translate({center})\" font-size=\"{}\" text-anchor=\"middle\" fill=\"var(--_bg)\">",
                num(small)
            );
            let r = radius * text_position;
            for (_, share, start, end) in inside {
                let at = point((start + end) / 2.0, r);
                let _ = write!(
                    content,
                    "<text x=\"{}\" y=\"{}\">{}</text>",
                    num(at.x),
                    num(at.y + small * 0.35),
                    percent(share)
                );
            }
            content.push_str("</g>");
        }
        if narrow.contains(&true) {
            let _ = write!(
                content,
                "<g class=\"{p}-slice-labels\" transform=\"translate({center})\" font-size=\"{}\" text-anchor=\"middle\">",
                num(small)
            );
            let rim = radius + OUTSIDE / 2.0;
            // Each label placed so far: its centre and its width.
            let mut placed_outside: Vec<(Point, f64)> = Vec::new();
            for (_, share, start, end) in arcs().filter(|&(index, ..)| narrow[index]) {
                let text = percent(share);
                let width = measure.width(&text, small, 0);
                let clear = |at: Point, placed: &[(Point, f64)]| {
                    placed.iter().all(|&(other, other_width)| {
                        (at.x - other.x).abs() >= (width + other_width) / 2.0 + 2.0
                            || (at.y - other.y).abs() >= small + 2.0
                    })
                };
                let mut angle = (start + end) / 2.0;
                let mut step = 0;
                while step < 360 && !clear(point(angle, rim), &placed_outside) {
                    angle += 2.0 / rim;
                    step += 1;
                }
                let at = point(angle, rim);
                placed_outside.push((at, width));
                let _ = write!(
                    content,
                    "<text x=\"{}\" y=\"{}\">{text}</text>",
                    num(at.x),
                    num(at.y + small * 0.35)
                );
            }
            content.push_str("</g>");
        }

        let legend_at = match legend_position {
            Some(LegendPosition::Right) => Point::new(
                center.x + diameter / 2.0 + GAP,
                body_top + (height - legend_height) / 2.0,
            ),
            Some(LegendPosition::Left) => {
                Point::new(pad, body_top + (height - legend_height) / 2.0)
            }
            Some(LegendPosition::Center) => Point::new(
                center.x - legend_width / 2.0,
                center.y - legend_height / 2.0,
            ),
            Some(LegendPosition::Top) => Point::new(pad + (width - legend_width) / 2.0, body_top),
            _ => Point::new(
                pad + (width - legend_width) / 2.0,
                body_top + diameter + 20.0,
            ),
        };
        if !legend.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-legend\" font-size=\"{}\">",
                num(small)
            );
            for (index, label) in legend.iter().enumerate() {
                let y = legend_at.y + index as f64 * ROW + ROW / 2.0;
                let _ = write!(
                    content,
                    "<g class=\"{p}-legend-item\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\" fill=\"{}\"/>{}</g>",
                    EscText(entries[index].0),
                    num(legend_at.x),
                    num(y - SWATCH / 2.0),
                    num(SWATCH),
                    num(SWATCH),
                    series_color(index as i64),
                    label.svg(
                        Point::new(legend_at.x + SWATCH + 8.0 + label.width / 2.0, y),
                        ""
                    )
                );
            }
            content.push_str("</g>");
        }

        Rendered {
            svg: svg_document(
                "pie",
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
