//! Pele's `diagrams/quadrant/render.ts`.

use crate::geom::{Point, Rect};
use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};

use super::db::{QuadrantModel, QuadrantText, TextKind};
use crate::config::capped;
use crate::svg::builder::EscText;
use crate::{Config, Ctx, Error, Host};
use crate::{
    Rendered,
    diagrams::Render,
    js,
    svg::{
        builder::{TITLE, fragment, num},
        root::svg_document,
        theme::{RADIUS, class_names, resolve_style, series_color},
    },
    text::{Label, Style},
};
use std::fmt::{self, Write as _};

const GAP: f64 = 4.0;
const CAREFUL: usize = 300;

struct Dot {
    center: Point,
    /// How far the circle and its stroke reach from the centre.
    reach: f64,
    text: Label,
}

fn hex(color: &str) -> impl fmt::Display + '_ {
    fragment(move |f| {
        if !color.starts_with('#') {
            f.write_str("#")?;
        }
        f.write_str(color)
    })
}

/// Mermaid's `quadrantChart` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct QuadrantConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub chart_height: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub chart_width: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub point_radius: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub point_text_padding: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub quadrant_padding: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub quadrant_text_top_padding: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub title_padding: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub x_axis_label_padding: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub y_axis_label_padding: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub x_axis_position: Option<XAxisPosition>,
    #[serde_as(as = "DefaultOnError")]
    pub y_axis_position: Option<YAxisPosition>,
}

/// Where the x axis labels go.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum XAxisPosition {
    Top,
    Bottom,
}

/// Where the y axis labels go.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum YAxisPosition {
    Left,
    Right,
}

impl Render for QuadrantModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(self.draw(&config.quadrant_chart, ctx))
    }
}

impl QuadrantModel {
    fn draw(&self, config: &QuadrantConfig, ctx: &Ctx<'_, impl Host>) -> Rendered {
        let p = crate::PREFIX;
        let size = ctx.options.font_size;
        let axis_size = js::round(size * 0.875);
        let point_size = js::round(size * 0.75);
        let measure = ctx.host;
        let natural = js::round(100f64.max(capped(config.chart_width, 500.0, 0.0, 4000.0)));
        let width = js::round(ctx.cap_width(natural, 280.0, 0.0));
        let height = js::round(
            (100f64.max(capped(config.chart_height, 500.0, 0.0, 4000.0)) * width) / natural,
        );
        let outer = capped(
            config.quadrant_padding,
            ctx.options.padding.unwrap_or(8.0),
            0.0,
            200.0,
        );
        let x_pad = capped(config.x_axis_label_padding, 5.0, 0.0, 200.0);
        let y_pad = capped(config.y_axis_label_padding, 5.0, 0.0, 200.0);
        let has_points = !self.points.is_empty();

        let label = |text: Option<&QuadrantText>, font_size: f64, max: f64| -> Label {
            Label::layout(
                text.map(|t| t.text.as_str()),
                text.is_some_and(|t| t.kind == TextKind::Markdown),
                &*measure,
                font_size,
                max.max(40.0),
                0,
            )
        };

        let title = Label::layout(
            self.titles.title.as_deref(),
            false,
            measure,
            size,
            (width - 2.0 * outer).max(40.0),
            Style::BOLD,
        );
        let title_pad = capped(config.title_padding, 10.0, 0.0, 200.0);
        let title_space = title.spaced_height(title_pad);

        let filled = |t: &Option<QuadrantText>| t.as_ref().is_some_and(|t| !t.text.is_empty());
        let has_x = filled(&self.x_axis_left) || filled(&self.x_axis_right);
        let has_y = filled(&self.y_axis_bottom) || filled(&self.y_axis_top);
        // Mermaid moves the x axis labels below the chart as soon as there are
        // points.
        let x_below = has_points || config.x_axis_position == Some(XAxisPosition::Bottom);
        let y_right = config.y_axis_position == Some(YAxisPosition::Right);

        let side = width - 2.0 * outer;
        let x_left = label(
            self.x_axis_left.as_ref(),
            axis_size,
            if self.x_axis_right.is_some() {
                side / 2.0 - 16.0
            } else {
                side
            },
        );
        let x_right = label(self.x_axis_right.as_ref(), axis_size, side / 2.0 - 16.0);
        let x_height = x_left.height.max(x_right.height);
        let x_space = if has_x { x_height + x_pad } else { 0.0 };
        let tall = height - 2.0 * outer - title_space - x_space;
        let y_bottom = label(
            self.y_axis_bottom.as_ref(),
            axis_size,
            if self.y_axis_top.is_some() {
                tall / 2.0 - 16.0
            } else {
                tall
            },
        );
        let y_top = label(self.y_axis_top.as_ref(), axis_size, tall / 2.0 - 16.0);
        let y_height = y_bottom.height.max(y_top.height);
        let y_space = if has_y { y_height + y_pad } else { 0.0 };

        let left = outer + if y_right { 0.0 } else { y_space };
        let top = outer + title_space + if x_below { 0.0 } else { x_space };
        let w = 0f64.max(side - y_space);
        let h = 0f64.max(tall);
        let tile_w = 0f64.max((w - GAP) / 2.0);
        let tile_h = 0f64.max((h - GAP) / 2.0);

        // Boxes a point label keeps clear of: the gaps between quadrants,
        // quadrant labels, points, and the labels placed before it.
        let mut taken: Vec<Rect> = vec![
            Rect::new(
                Point::new(left + tile_w, top),
                Point::new(left + w - tile_w, top + h),
            ),
            Rect::new(
                Point::new(left, top + tile_h),
                Point::new(left + w, top + h - tile_h),
            ),
        ];
        // The gaps between quadrants come first in `taken`.
        const GAPS: usize = 2;

        let text_top = capped(config.quadrant_text_top_padding, 5.0, 0.0, 200.0);
        let mut quadrants = String::new();
        for k in 0..4 {
            let x = if k == 0 || k == 3 {
                left + w - tile_w
            } else {
                left
            };
            let y = if k < 2 { top } else { top + h - tile_h };
            let text = label(self.quadrants[k].as_ref(), size, tile_w - 16.0);
            let center = Point::new(
                x + tile_w / 2.0,
                if has_points {
                    y + text_top + text.height / 2.0
                } else {
                    y + tile_h / 2.0
                },
            );
            taken.push(Rect::around(
                center,
                Point::new(text.width / 2.0, text.height / 2.0),
            ));
            let _ = write!(
                quadrants,
                "<g class=\"{p}-quadrant\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"/>{}</g>",
                k + 1,
                num(x),
                num(y),
                num(tile_w),
                num(tile_h),
                text.svg(
                    center,
                    crate::class!("label", "fill=\"var(--_fg)\" stroke=\"none\"")
                )
            );
        }

        let mut axes = String::new();
        if has_x {
            let y = if x_below {
                top + h + x_pad + x_height / 2.0
            } else {
                top - x_pad - x_height / 2.0
            };
            let _ = write!(
                axes,
                "{}",
                x_left.svg(
                    Point::new(
                        if x_right.height > 0.0 {
                            left + w / 4.0
                        } else {
                            left + x_left.width / 2.0
                        },
                        y
                    ),
                    ""
                )
            );
            let _ = write!(
                axes,
                "{}",
                x_right.svg(Point::new(left + (w * 3.0) / 4.0, y), "")
            );
        }
        if has_y {
            let x = if y_right {
                left + w + y_pad + y_height / 2.0
            } else {
                left - y_pad - y_height / 2.0
            };
            let mut turned = |text: &Label, y: f64| {
                if text.height > 0.0 {
                    let _ = write!(
                        axes,
                        "<g transform=\"translate({},{}) rotate(-90)\">{}</g>",
                        num(x),
                        num(y),
                        text.svg(Point::new(0.0, 0.0), "")
                    );
                }
            };
            turned(
                &y_bottom,
                if y_top.height > 0.0 {
                    top + (h * 3.0) / 4.0
                } else {
                    top + h - y_bottom.width / 2.0
                },
            );
            turned(&y_top, top + h / 4.0);
        }

        let radius = capped(config.point_radius, 5.0, 0.0, 200.0);
        let text_pad = capped(config.point_text_padding, 5.0, 0.0, 200.0);
        let fill = series_color(0);
        let mut dots: Vec<Dot> = Vec::new();
        let mut circles = String::new();
        // Where each circle in `circles` ends.
        let mut ends: Vec<usize> = Vec::new();
        for point in &self.points {
            if !(point.x >= 0.0 && point.x <= 1.0 && point.y >= 0.0 && point.y <= 1.0) {
                continue;
            }
            let shared = self.classes.get(&point.class_name);
            let r = point
                .style
                .radius
                .or_else(|| shared.and_then(|s| s.radius))
                .unwrap_or(radius)
                .min(2000.0);
            let color = point
                .style
                .color
                .as_ref()
                .or_else(|| shared.and_then(|s| s.color.as_ref()));
            let stroke_color = point
                .style
                .stroke_color
                .as_ref()
                .or_else(|| shared.and_then(|s| s.stroke_color.as_ref()));
            let stroke_width = point
                .style
                .stroke_width
                .as_ref()
                .or_else(|| shared.and_then(|s| s.stroke_width.as_ref()));
            let mut styles: Vec<String> = Vec::new();
            if let Some(color) = color.filter(|c| !c.is_empty()) {
                styles.push(format!("fill:{}", hex(color)));
            }
            let stroke_width = stroke_width.filter(|w| !w.is_empty());
            let stroke_color = stroke_color.filter(|c| !c.is_empty());
            if let Some(stroke_width) = stroke_width {
                if let Some(stroke_color) = stroke_color {
                    styles.push(format!("stroke:{}", hex(stroke_color)));
                }
                styles.push(format!("stroke-width:{stroke_width}"));
            }
            let center = Point::new(left + point.x * w, top + (1.0 - point.y) * h);
            dots.push(Dot {
                center,
                reach: r + stroke_width.map_or(0.0, |w| js::parse_int(w, 10).min(400.0) / 2.0),
                text: label(Some(&point.text), point_size, 200.0),
            });
            let _ = write!(
                circles,
                "<circle class=\"{p}-point{}\" data-id=\"{}\" cx=\"{}\" cy=\"{}\" r=\"{}\"{}{}/>",
                class_names(&point.class_name),
                EscText(&point.text.text),
                num(center.x),
                num(center.y),
                num(r),
                fragment(|f| if stroke_width.is_some() && stroke_color.is_none() {
                    write!(f, " stroke=\"{fill}\"")
                } else {
                    Ok(())
                }),
                resolve_style(&styles).shape
            );
            ends.push(circles.len());
        }

        // A label goes below its point, or above, right or left of it when that is
        // the first place that is inside the chart and free. A second round lets it
        // cross the gaps between quadrants. Large charts skip the search for a free
        // place.
        let careful = dots.len() <= CAREFUL;
        if careful {
            for dot in &dots {
                taken.push(Rect::around(dot.center, Point::new(dot.reach, dot.reach)));
            }
        }
        let mut labels = String::new();
        for dot in &dots {
            let (center, text) = (dot.center, &dot.text);
            let half_w = text.width / 2.0;
            let half_h = text.size * 0.6 + (text.height - text.line_height) / 2.0;
            let half = Point::new(half_w, half_h);
            let away = dot.reach + text_pad;
            // Below, above, right and left of the point.
            let spots = [
                Point::new(center.x, center.y + away + half_h),
                Point::new(center.x, center.y - away - half_h),
                Point::new(center.x + away + half_w, center.y),
                Point::new(center.x - away - half_w, center.y),
            ];
            let mut at = spots[0];
            let mut found = false;
            'search: for round in 0..2 {
                for (index, &spot) in spots.iter().enumerate() {
                    let x = if index < 2 && text.width < w {
                        spot.x.max(left + half_w).min(left + w - half_w)
                    } else {
                        spot.x
                    };
                    let y = spot.y;
                    if x - half_w < left
                        || x + half_w > left + w
                        || y - half_h < top
                        || y + half_h > top + h
                    {
                        continue;
                    }
                    let place = Rect::around(Point::new(x, y), half);
                    let from = if round == 0 { 0 } else { GAPS };
                    let free = !careful || !taken[from..].iter().any(|t| place.overlaps(*t));
                    if !found || free {
                        at = Point::new(x, y);
                        found = true;
                    }
                    if free {
                        break 'search;
                    }
                }
            }
            if careful {
                taken.push(Rect::around(at, half));
            }
            let _ = write!(labels, "{}", text.svg(at, crate::class!("label")));
        }

        // Mermaid puts each new point under the ones before it.
        let mut content = String::with_capacity(
            quadrants.len() + axes.len() + circles.len() + labels.len() + 1024,
        );
        let _ = write!(
            content,
            "{}",
            title.svg(
                Point::new(width / 2.0, title_pad / 2.0 + title.height / 2.0),
                TITLE
            )
        );
        let _ = write!(
            content,
            "<g class=\"{p}-quadrants\" fill=\"var(--_s)\" stroke=\"var(--_b)\">{quadrants}</g>"
        );
        if !axes.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-axis-labels\" font-size=\"{}\" fill=\"var(--_m)\">{axes}</g>",
                js::Number(axis_size)
            );
        }
        if !dots.is_empty() {
            let _ = write!(content, "<g class=\"{p}-points\" fill=\"{fill}\">");
            for (i, &end) in ends.iter().enumerate().rev() {
                let start = if i == 0 { 0 } else { ends[i - 1] };
                content.push_str(&circles[start..end]);
            }
            let _ = write!(
                content,
                "</g><g class=\"{p}-point-labels\" font-size=\"{}\">{labels}</g>",
                js::Number(point_size)
            );
        }

        Rendered {
            svg: svg_document(
                "quadrantChart",
                width,
                height,
                size,
                ctx,
                self.titles.accessible(),
                &content,
            ),
            width,
            height,
            links: Vec::new(),
        }
    }
}
