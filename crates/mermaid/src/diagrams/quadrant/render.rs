//! Pele's `diagrams/quadrant/render.ts`.

use super::db::{QuadrantModel, QuadrantText};
use crate::{
    Config, Rendered,
    config::Value,
    diagrams::common::fit_width::cap_width,
    js,
    svg::{
        builder::{esc_text, label_svg, num},
        root::{Accessible, svg_document},
        theme::{RADIUS, class_names, resolve_style, series_color},
    },
    text::{Label, Style, layout_label},
};
use crate::{Ctx, Host};
use std::fmt::Write as _;

const GAP: f64 = 4.0;
const CAREFUL: usize = 300;

struct Dot {
    cx: f64,
    cy: f64,
    reach: f64,
    text: Label,
}

fn setting<'c>(config: &'c Config, key: &str) -> Option<&'c Value> {
    config.section("quadrantChart").and_then(|q| q.get(key))
}

fn length(config: &Config, key: &str, fallback: f64, max: f64) -> f64 {
    match setting(config, key) {
        Some(Value::Number(value)) if *value >= 0.0 => value.min(max),
        _ => fallback,
    }
}

fn hex(color: &str) -> String {
    if color.starts_with('#') {
        color.to_string()
    } else {
        format!("#{color}")
    }
}

pub fn render_quadrant(
    model: &QuadrantModel,
    config: &Config,
    ctx: &Ctx<'_, impl Host>,
) -> Rendered {
    let p = crate::PREFIX;
    let size = ctx.options.font_size;
    let axis_size = js::round(size * 0.875);
    let point_size = js::round(size * 0.75);
    let measure = ctx.host;
    let natural = js::round(100f64.max(length(config, "chartWidth", 500.0, 4000.0)));
    let width = js::round(cap_width(ctx, natural, 280.0, 0.0));
    let height =
        js::round((100f64.max(length(config, "chartHeight", 500.0, 4000.0)) * width) / natural);
    let outer = length(
        config,
        "quadrantPadding",
        ctx.options.padding.unwrap_or(8.0),
        200.0,
    );
    let x_pad = length(config, "xAxisLabelPadding", 5.0, 200.0);
    let y_pad = length(config, "yAxisLabelPadding", 5.0, 200.0);
    let has_points = !model.points.is_empty();

    let label = |text: Option<&QuadrantText>, font_size: f64, max: f64| -> Label {
        layout_label(
            text.map(|t| t.text.as_str()),
            text.is_some_and(|t| t.kind == "markdown"),
            &*measure,
            font_size,
            max.max(40.0),
            0,
        )
    };

    let title = layout_label(
        model.title.as_deref(),
        false,
        measure,
        size,
        (width - 2.0 * outer).max(40.0),
        Style::BOLD,
    );
    let title_pad = length(config, "titlePadding", 10.0, 200.0);
    let title_space = if title.height > 0.0 {
        title.height + title_pad
    } else {
        0.0
    };

    let filled = |t: &Option<QuadrantText>| t.as_ref().is_some_and(|t| !t.text.is_empty());
    let has_x = filled(&model.x_axis_left) || filled(&model.x_axis_right);
    let has_y = filled(&model.y_axis_bottom) || filled(&model.y_axis_top);
    // Mermaid moves the x axis labels below the chart as soon as there are
    // points.
    let x_below =
        has_points || setting(config, "xAxisPosition").and_then(Value::as_str) == Some("bottom");
    let y_right = setting(config, "yAxisPosition").and_then(Value::as_str) == Some("right");

    let side = width - 2.0 * outer;
    let x_left = label(
        model.x_axis_left.as_ref(),
        axis_size,
        if model.x_axis_right.is_some() {
            side / 2.0 - 16.0
        } else {
            side
        },
    );
    let x_right = label(model.x_axis_right.as_ref(), axis_size, side / 2.0 - 16.0);
    let x_height = x_left.height.max(x_right.height);
    let x_space = if has_x { x_height + x_pad } else { 0.0 };
    let tall = height - 2.0 * outer - title_space - x_space;
    let y_bottom = label(
        model.y_axis_bottom.as_ref(),
        axis_size,
        if model.y_axis_top.is_some() {
            tall / 2.0 - 16.0
        } else {
            tall
        },
    );
    let y_top = label(model.y_axis_top.as_ref(), axis_size, tall / 2.0 - 16.0);
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
    let mut taken: Vec<f64> = vec![
        left + tile_w,
        top,
        left + w - tile_w,
        top + h,
        left,
        top + tile_h,
        left + w,
        top + h - tile_h,
    ];
    let is_clear = |taken: &[f64], from: usize, x0: f64, y0: f64, x1: f64, y1: f64| -> bool {
        let mut k = from;
        while k < taken.len() {
            if x0 < taken[k + 2] && x1 > taken[k] && y0 < taken[k + 3] && y1 > taken[k + 1] {
                return false;
            }
            k += 4;
        }
        true
    };

    let text_top = length(config, "quadrantTextTopPadding", 5.0, 200.0);
    let mut quadrants = String::new();
    for k in 0..4 {
        let x = if k == 0 || k == 3 {
            left + w - tile_w
        } else {
            left
        };
        let y = if k < 2 { top } else { top + h - tile_h };
        let text = label(model.quadrants[k].as_ref(), size, tile_w - 16.0);
        let cx = x + tile_w / 2.0;
        let cy = if has_points {
            y + text_top + text.height / 2.0
        } else {
            y + tile_h / 2.0
        };
        taken.extend([
            cx - text.width / 2.0,
            cy - text.height / 2.0,
            cx + text.width / 2.0,
            cy + text.height / 2.0,
        ]);
        let _ = write!(
            quadrants,
            "<g class=\"{p}-quadrant\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"/>{}</g>",
            k + 1,
            num(x),
            num(y),
            num(tile_w),
            num(tile_h),
            label_svg(
                &text,
                cx,
                cy,
                &format!(" class=\"{p}-label\" fill=\"var(--_fg)\" stroke=\"none\"")
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
        axes.push_str(&label_svg(
            &x_left,
            if x_right.height > 0.0 {
                left + w / 4.0
            } else {
                left + x_left.width / 2.0
            },
            y,
            "",
        ));
        axes.push_str(&label_svg(&x_right, left + (w * 3.0) / 4.0, y, ""));
    }
    if has_y {
        let x = if y_right {
            left + w + y_pad + y_height / 2.0
        } else {
            left - y_pad - y_height / 2.0
        };
        let turned = |text: &Label, y: f64| -> String {
            if text.height > 0.0 {
                format!(
                    "<g transform=\"translate({},{}) rotate(-90)\">{}</g>",
                    num(x),
                    num(y),
                    label_svg(text, 0.0, 0.0, "")
                )
            } else {
                String::new()
            }
        };
        axes.push_str(&turned(
            &y_bottom,
            if y_top.height > 0.0 {
                top + (h * 3.0) / 4.0
            } else {
                top + h - y_bottom.width / 2.0
            },
        ));
        axes.push_str(&turned(&y_top, top + h / 4.0));
    }

    let radius = length(config, "pointRadius", 5.0, 200.0);
    let text_pad = length(config, "pointTextPadding", 5.0, 200.0);
    let fill = series_color(0);
    let mut dots: Vec<Dot> = Vec::new();
    let mut circles: Vec<String> = Vec::new();
    for point in &model.points {
        if !(point.x >= 0.0 && point.x <= 1.0 && point.y >= 0.0 && point.y <= 1.0) {
            continue;
        }
        let shared = model.classes.get(&point.class_name);
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
        let cx = left + point.x * w;
        let cy = top + (1.0 - point.y) * h;
        dots.push(Dot {
            cx,
            cy,
            reach: r + stroke_width.map_or(0.0, |w| js::parse_int(w, 10).min(400.0) / 2.0),
            text: label(Some(&point.text), point_size, 200.0),
        });
        circles.push(format!(
            "<circle class=\"{p}-point{}\" data-id=\"{}\" cx=\"{}\" cy=\"{}\" r=\"{}\"{}{}/>",
            class_names(&point.class_name),
            esc_text(&point.text.text),
            num(cx),
            num(cy),
            num(r),
            if stroke_width.is_some() && stroke_color.is_none() {
                format!(" stroke=\"{fill}\"")
            } else {
                String::new()
            },
            resolve_style(&styles).shape
        ));
    }

    // A label goes below its point, or above, right or left of it when that is
    // the first place that is inside the chart and free. A second round lets it
    // cross the gaps between quadrants. Large charts skip the search for a free
    // place.
    let careful = dots.len() <= CAREFUL;
    if careful {
        for dot in &dots {
            taken.extend([
                dot.cx - dot.reach,
                dot.cy - dot.reach,
                dot.cx + dot.reach,
                dot.cy + dot.reach,
            ]);
        }
    }
    let mut labels = String::new();
    for dot in &dots {
        let (cx, cy, text) = (dot.cx, dot.cy, &dot.text);
        let half_w = text.width / 2.0;
        let half_h = text.size * 0.6 + (text.height - text.line_height) / 2.0;
        let away = dot.reach + text_pad;
        let spots = [
            cx,
            cy + away + half_h,
            cx,
            cy - away - half_h,
            cx + away + half_w,
            cy,
            cx - away - half_w,
            cy,
        ];
        let mut tx = cx;
        let mut ty = spots[1];
        let mut found = false;
        let mut k = 0;
        while k < 16 {
            let spot = k % 8;
            let x = if spot < 4 && text.width < w {
                spots[spot].max(left + half_w).min(left + w - half_w)
            } else {
                spots[spot]
            };
            let y = spots[spot + 1];
            if x - half_w < left
                || x + half_w > left + w
                || y - half_h < top
                || y + half_h > top + h
            {
                k += 2;
                continue;
            }
            let free = !careful
                || is_clear(
                    &taken,
                    if k < 8 { 0 } else { 8 },
                    x - half_w,
                    y - half_h,
                    x + half_w,
                    y + half_h,
                );
            if !found || free {
                tx = x;
                ty = y;
                found = true;
            }
            if free {
                break;
            }
            k += 2;
        }
        if careful {
            taken.extend([tx - half_w, ty - half_h, tx + half_w, ty + half_h]);
        }
        labels.push_str(&label_svg(text, tx, ty, &format!(" class=\"{p}-label\"")));
    }

    // Mermaid puts each new point under the ones before it.
    let mut content = label_svg(
        &title,
        width / 2.0,
        title_pad / 2.0 + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
    );
    let _ = write!(
        content,
        "<g class=\"{p}-quadrants\" fill=\"var(--_s)\" stroke=\"var(--_b)\">{quadrants}</g>"
    );
    if !axes.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-axis-labels\" font-size=\"{}\" fill=\"var(--_m)\">{axes}</g>",
            js::num_str(axis_size)
        );
    }
    if !dots.is_empty() {
        circles.reverse();
        let _ = write!(
            content,
            "<g class=\"{p}-points\" fill=\"{fill}\">{}</g><g class=\"{p}-point-labels\" font-size=\"{}\">{labels}</g>",
            circles.concat(),
            js::num_str(point_size)
        );
    }

    Rendered {
        svg: svg_document(
            "quadrantChart",
            width,
            height,
            size,
            ctx,
            Accessible {
                acc_title: model.acc_title.as_deref(),
                acc_descr: model.acc_descr.as_deref(),
            },
            &content,
        ),
        width,
        height,
        links: Vec::new(),
    }
}
