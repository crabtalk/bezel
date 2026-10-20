//! Pele's `diagrams/wardley/render.ts`.

use crate::config::within;
use crate::geom::Point;
use crate::svg::edges::Marker;
use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    f64::consts::{PI, SQRT_2},
    fmt::{self, Write as _},
};

use indexmap::IndexMap;

use super::model::WardleyModel;
use crate::{Ctx, Host};
use crate::{
    Rendered,
    diagrams::common::fit_width::fit_width,
    js,
    svg::{
        builder::{Esc, EscText, fragment, num, span_style, text_style},
        root::{Accessible, svg_document},
        theme::RADIUS,
    },
    text::{Label, Style},
};

const DEFAULT_STAGES: [&str; 4] = ["Genesis", "Custom Built", "Product", "Commodity"];
/// Mermaid's canvas has this much room around the plot on every side.
const CANVAS_MARGIN: f64 = 48.0;
const R: f64 = 6.0;
const SQUARE: f64 = 10.0;
const LABEL_OFFSET: f64 = 8.0;
const PIPELINE_PAD: f64 = 15.0;
const FORCE_WIDTH: f64 = 60.0;
const FORCE_HEIGHT: f64 = 30.0;
const FORCE_HEAD: f64 = 20.0;
const ACCENT: &str = "var(--_c)";
const LINE: &str = "var(--_l)";

struct Placed {
    x: f64,
    y: f64,
    /// The node's index in the model.
    node: usize,
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// A label with its lines starting at x and its first baseline at y.
struct StartLabel<'a>(&'a Label, f64, f64, &'a str);

impl fmt::Display for StartLabel<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let StartLabel(label, x, y, attrs) = *self;
        if !label.lines.iter().flatten().any(|span| span.icon.is_none()) {
            return Ok(());
        }
        let shared = label.shared_style();
        write!(
            f,
            "<text{} xml:space=\"preserve\">",
            text_style(attrs, shared)
        )?;
        let mut baseline = y;
        for line in &label.lines {
            let mut first = true;
            for span in line {
                if span.icon.is_some() {
                    continue;
                }
                let style = span_style(span.style & !shared);
                if first {
                    write!(
                        f,
                        "<tspan x=\"{}\" y=\"{}\"{style}>{}</tspan>",
                        num(x),
                        num(baseline),
                        Esc(&span.text)
                    )?;
                } else {
                    write!(f, "<tspan{style}>{}</tspan>", Esc(&span.text))?;
                }
                first = false;
            }
            baseline += label.line_height;
        }
        f.write_str("</text>")
    }
}

fn dimension(given: Option<f64>, configured: Option<f64>, fallback: f64) -> f64 {
    let value = match given {
        Some(given) if given.is_finite() => given,
        _ => within(configured, fallback, 1.0, 1e6),
    };
    clamp(value, 200.0, 10000.0) - 2.0 * CANVAS_MARGIN
}

/// Mermaid's `wardley-beta` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WardleyConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub width: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub height: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub show_grid: Option<bool>,
}

pub fn render_wardley(
    model: &WardleyModel,
    config: &WardleyConfig,
    ctx: &Ctx<'_, impl Host>,
) -> Rendered {
    let natural = dimension(model.built.size.map(|size| size.width), config.width, 900.0);
    fit_width(ctx, natural, 160.0, |plot_w, tight| {
        draw(model, config, ctx, plot_w, natural, tight)
    })
}

struct Bounds {
    min_x: Cell<f64>,
    min_y: Cell<f64>,
    max_x: Cell<f64>,
    max_y: Cell<f64>,
}

impl Bounds {
    fn grow(&self, x0: f64, y0: f64, x1: f64, y1: f64) {
        if x0 < self.min_x.get() {
            self.min_x.set(x0);
        }
        if y0 < self.min_y.get() {
            self.min_y.set(y0);
        }
        if x1 > self.max_x.get() {
            self.max_x.set(x1);
        }
        if y1 > self.max_y.get() {
            self.max_y.set(y1);
        }
    }
}

fn draw(
    model: &WardleyModel,
    config: &WardleyConfig,
    ctx: &Ctx<'_, impl Host>,
    plot_w: f64,
    natural_w: f64,
    tight: bool,
) -> Rendered {
    let p = crate::PREFIX;
    let built = &model.built;
    let size = ctx.options.font_size;
    let small = js::round(size * 0.8125);
    let tiny = js::round(size * 0.6875);
    let measure = ctx.host;
    let pad = ctx.options.padding.unwrap_or(8.0);

    // A narrower map is not as much shorter, so that components keep some
    // room between them.
    let plot_h = dimension(built.size.map(|size| size.height), config.height, 600.0)
        * 0.75f64.max(plot_w / natural_w);
    let show_grid = config.show_grid == Some(true);

    let bounds = Bounds {
        min_x: Cell::new(0.0),
        min_y: Cell::new(0.0),
        max_x: Cell::new(plot_w),
        max_y: Cell::new(plot_h),
    };
    let grow = |x0: f64, y0: f64, x1: f64, y1: f64| bounds.grow(x0, y0, x1, y1);
    let centered = |out: &mut String, label: &Label, cx: f64, cy: f64, attrs: &str| {
        grow(
            cx - label.width / 2.0,
            cy - label.height / 2.0,
            cx + label.width / 2.0,
            cy + label.height / 2.0,
        );
        let _ = write!(out, "{}", label.svg(Point::new(cx, cy), attrs));
    };
    let starting = |out: &mut String, label: &Label, x: f64, y: f64, attrs: &str| {
        grow(
            x,
            y - label.size,
            x + label.width,
            y - label.size + label.height + 4.0,
        );
        let _ = write!(out, "{}", StartLabel(label, x, y, attrs));
    };
    let default_width = if tight {
        64f64.max(120f64.min(plot_w / 2.0))
    } else {
        4000.0
    };
    let text = |raw: &str, font_size: f64, max_width: Option<f64>, style: u8| -> Label {
        Label::layout(
            Some(raw),
            false,
            measure,
            font_size,
            max_width.unwrap_or(default_width),
            style,
        )
    };
    let project_x = |value: f64| (value / 100.0) * plot_w;
    let project_y = |value: f64| plot_h - (value / 100.0) * plot_h;

    let stages: Vec<&str> = match &built.axes.stages {
        Some(stages) if !stages.is_empty() => stages.iter().map(String::as_str).collect(),
        _ => DEFAULT_STAGES.to_vec(),
    };
    let boundaries = built.axes.stage_boundaries.as_ref();
    let custom = boundaries.is_some_and(|b| b.len() == stages.len());
    let mut frame = String::new();
    let mut stage_labels = String::new();
    let mut stage_bottom = plot_h;
    let mut previous = 0.0;
    let count = stages.len() as f64;
    let stage_attrs = format!(
        " class=\"{p}-wardley-stage-label\" font-size=\"{}\" fill=\"var(--_m)\"",
        num(small)
    );
    for (index, stage) in stages.iter().enumerate() {
        let start = if custom {
            previous
        } else {
            index as f64 / count
        };
        let end = if custom {
            clamp(boundaries.unwrap()[index], 0.0, 1.0)
        } else {
            (index + 1) as f64 / count
        };
        previous = end;
        if index > 0 {
            let _ = write!(
                frame,
                "<path class=\"{p}-wardley-stage\" d=\"M{},0V{}\" stroke=\"var(--_b)\" stroke-dasharray=\"4 4\"/>",
                num(start * plot_w),
                num(plot_h)
            );
        }
        let label = text(
            stage,
            small,
            Some(48f64.max((end - start).abs() * plot_w - 12.0)),
            0,
        );
        centered(
            &mut stage_labels,
            &label,
            ((start + end) / 2.0) * plot_w,
            plot_h + 8.0 + label.height / 2.0,
            &stage_attrs,
        );
        stage_bottom = stage_bottom.max(plot_h + 8.0 + label.height);
    }
    if show_grid {
        for i in 1..4 {
            let i = f64::from(i);
            let _ = write!(
                frame,
                "<path class=\"{p}-wardley-grid\" d=\"M{},0V{}M0,{}H{}\" stroke=\"var(--_b)\" stroke-dasharray=\"2 6\"/>",
                num((plot_w * i) / 4.0),
                num(plot_h),
                num((plot_h * i) / 4.0),
                num(plot_w)
            );
        }
    }
    let _ = write!(
        frame,
        "<path class=\"{p}-wardley-axis\" d=\"M0,0V{}H{}\" stroke=\"{LINE}\"/>",
        num(plot_h),
        num(plot_w)
    );
    let x_title = text(
        built.axes.x_label.as_deref().unwrap_or("Evolution"),
        small,
        None,
        0,
    );
    let y_title = text(
        built.axes.y_label.as_deref().unwrap_or("Visibility"),
        small,
        None,
        0,
    );
    let axis_attrs = format!(
        " class=\"{p}-wardley-axis-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"",
        num(small)
    );
    let mut axis_labels = String::new();
    centered(
        &mut axis_labels,
        &x_title,
        plot_w / 2.0,
        stage_bottom + 6.0 + x_title.height / 2.0,
        &axis_attrs,
    );
    let turned = format!(
        "{axis_attrs} transform=\"rotate(-90 -16 {})\"",
        num(plot_h / 2.0)
    );
    let _ = write!(
        axis_labels,
        "{}",
        y_title.svg(Point::new(-16.0, plot_h / 2.0), &turned)
    );
    grow(-16.0 - y_title.height / 2.0, 0.0, 0.0, 0.0);
    let legend_top = stage_bottom + 6.0 + x_title.height + 10.0;

    let mut positions: IndexMap<&str, Placed> = IndexMap::new();
    for (index, node) in built.nodes.iter().enumerate() {
        positions.insert(
            &node.id,
            Placed {
                x: project_x(node.x),
                y: project_y(node.y),
                node: index,
            },
        );
    }

    // Pipelines: a box around the components, with the parent as a square
    // on its top edge.
    let mut pipelines = String::new();
    let mut members: HashMap<&str, HashSet<&str>> = HashMap::new();
    for pipeline in &built.pipelines {
        members.insert(
            &pipeline.node_id,
            pipeline.component_ids.iter().map(String::as_str).collect(),
        );
        let mut parts: Vec<(f64, f64)> = Vec::new();
        for id in &pipeline.component_ids {
            if let Some(part) = positions.get(id.as_str()) {
                parts.push((part.x, part.y));
            }
        }
        if parts.is_empty() {
            continue;
        }
        parts.sort_by(|a, b| js::compare(a.0 - b.0));
        let left = parts[0].0;
        let right = parts[parts.len() - 1].0;
        let y = parts[0].1;
        let top = y - 2.0 * R;
        if let Some(parent) = positions.get_mut(pipeline.node_id.as_str()) {
            parent.x = (left + right) / 2.0;
            parent.y = top - SQUARE / 6.0;
        }
        grow(
            left - PIPELINE_PAD,
            top,
            right + PIPELINE_PAD,
            top + 4.0 * R,
        );
        let _ = write!(
            pipelines,
            "<g class=\"{p}-wardley-pipeline\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"/>{}</g>",
            EscText(&pipeline.node_id),
            num(left - PIPELINE_PAD),
            num(top),
            num(right - left + 2.0 * PIPELINE_PAD),
            num(4.0 * R),
            fragment(|f| if parts.len() > 1 {
                write!(
                    f,
                    "<path d=\"M{}H{}\" stroke=\"{LINE}\" stroke-dasharray=\"4 4\"/>",
                    Point::new(left, y),
                    num(right)
                )
            } else {
                Ok(())
            })
        );
    }

    // How far from a node's center a line to it stops.
    let anchor_labels: RefCell<HashMap<usize, Label>> = RefCell::new(HashMap::new());
    let anchor_width = if tight {
        64f64.max(plot_w / 2.0)
    } else {
        4000.0
    };
    let reach = |at: &Placed, ux: f64, uy: f64| -> f64 {
        let node = &built.nodes[at.node];
        if node.class_name.as_deref() == Some("anchor") {
            let mut labels = anchor_labels.borrow_mut();
            let label = labels
                .entry(at.node)
                .or_insert_with(|| text(&node.label, small, Some(anchor_width), Style::BOLD));
            return 1.0 / js::hypot(ux / (label.width / 2.0 + 5.0), uy / (small * 0.8));
        }
        if node.is_pipeline_parent == Some(true) {
            return SQUARE / SQRT_2;
        }
        if node
            .source_strategy
            .as_deref()
            .is_some_and(|s| !s.is_empty())
        {
            2.0 * R
        } else {
            R
        }
    };

    // Quadrants around a node that a link runs through: 1 up and right, 2
    // down and right, 4 up and left, 8 down and left.
    let mut taken: HashMap<usize, u8> = HashMap::new();
    let mut occupy = |at: usize, ux: f64, uy: f64| {
        if ux.abs() < 0.25 || uy.abs() < 0.25 {
            return;
        }
        let bit = if ux > 0.0 {
            if uy < 0.0 { 1 } else { 2 }
        } else if uy < 0.0 {
            4
        } else {
            8
        };
        *taken.entry(at).or_insert(0) |= bit;
    };

    let mut links = String::new();
    let caption_attrs = format!(
        " class=\"{p}-edge-label\" font-size=\"{}\" fill=\"var(--_m)\"",
        num(tiny + 1.0)
    );
    for link in &built.links {
        let (Some(fi), Some(ti)) = (
            positions.get_index_of(link.source.as_str()),
            positions.get_index_of(link.target.as_str()),
        ) else {
            continue;
        };
        if fi == ti {
            continue;
        }
        // A pipeline component belongs to its parent already.
        if members
            .get(link.target.as_str())
            .is_some_and(|set| set.contains(link.source.as_str()))
        {
            continue;
        }
        let from = &positions[fi];
        let to = &positions[ti];
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let distance = js::hypot(dx, dy);
        if distance < 0.01 {
            continue;
        }
        let ux = dx / distance;
        let uy = dy / distance;
        let start = reach(from, ux, uy);
        let end = reach(to, ux, uy);
        if start + end >= distance {
            continue;
        }
        occupy(fi, ux, uy);
        occupy(ti, -ux, -uy);
        let mut x1 = from.x + ux * start;
        let mut y1 = from.y + uy * start;
        let mut x2 = to.x - ux * end;
        let mut y2 = to.y - uy * end;
        let flow = link.flow.unwrap_or("");
        let forward = flow == "forward" || flow == "bidirectional";
        let backward = flow == "backward" || flow == "bidirectional";
        let (head_x, head_y, tail_x, tail_y) = (x2, y2, x1, y1);
        if forward {
            x2 -= ux * Marker::Arrow.trim();
            y2 -= uy * Marker::Arrow.trim();
        }
        if backward {
            x1 += ux * Marker::Arrow.trim();
            y1 += uy * Marker::Arrow.trim();
        }
        let _ = write!(
            links,
            "<g class=\"{p}-edge{}\" data-id=\"{}-{}\"><path d=\"M{}L{}\" stroke=\"{LINE}\"{}/>",
            fragment(|f| if link.dashed {
                write!(f, " {p}-wardley-dashed")
            } else {
                Ok(())
            }),
            EscText(&link.source),
            EscText(&link.target),
            Point::new(x1, y1),
            Point::new(x2, y2),
            if link.dashed {
                " stroke-dasharray=\"6 6\""
            } else {
                ""
            }
        );
        if forward {
            let _ = write!(
                links,
                "{}",
                Marker::Arrow.draw(Point::new(head_x, head_y), Point::new(ux, uy), LINE)
            );
        }
        if backward {
            let _ = write!(
                links,
                "{}",
                Marker::Arrow.draw(Point::new(tail_x, tail_y), Point::new(-ux, -uy), LINE)
            );
        }
        if let Some(raw) = link.label.as_deref().filter(|l| !l.is_empty()) {
            let label = text(raw, tiny + 1.0, Some(200.0), 0);
            // Beside the middle of the line, on its upper side.
            let side = if ux >= 0.0 { 1.0 } else { -1.0 };
            let px = uy * side;
            let py = -ux * side;
            let away = 5.0 + (px.abs() * label.width) / 2.0 + (py.abs() * label.height) / 2.0;
            centered(
                &mut links,
                &label,
                (from.x + to.x) / 2.0 + px * away,
                (from.y + to.y) / 2.0 + py * away,
                &caption_attrs,
            );
        }
        links.push_str("</g>");
    }

    // Evolution: a dashed arrow to where the component is heading, drawn as
    // an open circle.
    let mut trends = String::new();
    for trend in &built.trends {
        let Some(origin) = positions.get(trend.node_id.as_str()) else {
            continue;
        };
        let tx = project_x(trend.target_x);
        let ty = project_y(trend.target_y);
        let dx = tx - origin.x;
        let dy = ty - origin.y;
        let distance = js::hypot(dx, dy);
        let start = if distance > 0.0 {
            reach(origin, dx / distance, dy / distance)
        } else {
            0.0
        };
        grow(tx - R, ty - R, tx + R, ty + R);
        let _ = write!(
            trends,
            "<g class=\"{p}-wardley-trend\" data-id=\"{}\">",
            EscText(&trend.node_id)
        );
        if distance > start + R + 4.0 {
            let ux = dx / distance;
            let uy = dy / distance;
            let ex = tx - ux * (R + 2.0);
            let ey = ty - uy * (R + 2.0);
            let _ = write!(
                trends,
                "<path d=\"M{}L{}\" stroke=\"{ACCENT}\" stroke-dasharray=\"4 4\"/>{}",
                Point::new(origin.x + ux * start, origin.y + uy * start),
                Point::new(
                    ex - ux * Marker::Arrow.trim(),
                    ey - uy * Marker::Arrow.trim()
                ),
                Marker::Arrow.draw(Point::new(ex, ey), Point::new(ux, uy), ACCENT)
            );
        }
        let _ = write!(
            trends,
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_bg)\" stroke=\"{ACCENT}\"/></g>",
            num(tx),
            num(ty),
            num(R)
        );
    }

    let mut nodes = String::new();
    let anchor_attrs = format!(
        " class=\"{p}-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"",
        num(small)
    );
    let node_attrs = format!(" class=\"{p}-label\" font-size=\"{}\"", num(small));
    let tri = 1.2 * R;
    let dot = 0.7 * R;
    for (index, at) in positions.values().enumerate() {
        let node = &built.nodes[at.node];
        let (x, y) = (at.x, at.y);
        let anchor = node.class_name.as_deref() == Some("anchor");
        let strategy = node.source_strategy.as_deref().filter(|s| !s.is_empty());
        let pipeline_parent = node.is_pipeline_parent == Some(true);
        let in_pipeline = node.in_pipeline == Some(true);
        let class = if anchor {
            "anchor"
        } else if pipeline_parent {
            "pipeline-parent"
        } else if in_pipeline {
            "pipeline-component"
        } else {
            "component"
        };
        let _ = write!(
            nodes,
            "<g class=\"{p}-node {p}-wardley-{class}\" data-id=\"{}\">",
            EscText(&node.id)
        );
        if let Some(strategy) = strategy {
            let fill = match strategy {
                "outsource" => "var(--_m)",
                "buy" => "var(--_a)",
                "build" => "var(--_s)",
                _ => "var(--_bg)",
            };
            let _ = write!(
                nodes,
                "<circle class=\"{p}-wardley-{strategy}\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{fill}\" stroke=\"{LINE}\"/>",
                num(x),
                num(y),
                num(2.0 * R)
            );
        }
        if strategy == Some("market") {
            let bx = tri * (PI / 6.0).cos();
            let by = y + tri * (PI / 6.0).sin();
            let _ = write!(
                nodes,
                "<path d=\"M{},{}L{},{}H{}Z\" fill=\"none\" stroke=\"{LINE}\"/>",
                num(x),
                num(y - tri),
                num(x - bx),
                num(by),
                num(x + bx)
            );
            for (cx, cy) in [(x, y - tri), (x - bx, by), (x + bx, by)] {
                let _ = write!(
                    nodes,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_bg)\" stroke=\"{LINE}\"/>",
                    num(cx),
                    num(cy),
                    num(dot)
                );
            }
        } else if pipeline_parent {
            let _ = write!(
                nodes,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_bg)\" stroke=\"{LINE}\"/>",
                num(x - SQUARE / 2.0),
                num(y - SQUARE / 2.0),
                num(SQUARE),
                num(SQUARE)
            );
        } else if !anchor {
            let _ = write!(
                nodes,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_bg)\" stroke=\"{LINE}\"/>",
                num(x),
                num(y),
                num(R)
            );
        }
        if node.inertia == Some(true) {
            let offset = (if pipeline_parent { SQUARE / 2.0 } else { R })
                + 10.0
                + (if strategy.is_some() { R + 4.0 } else { 0.0 });
            let _ = write!(
                nodes,
                "<rect class=\"{p}-wardley-inertia\" x=\"{}\" y=\"{}\" width=\"4\" height=\"{}\" fill=\"{LINE}\"/>",
                num(x + offset),
                num(y - R),
                num(2.0 * R)
            );
        }
        let ox = node.label_offset_x.map(|v| clamp(v, -5000.0, 5000.0));
        let oy = node.label_offset_y.map(|v| clamp(v, -5000.0, 5000.0));
        if anchor {
            let label = anchor_labels
                .borrow()
                .get(&at.node)
                .cloned()
                .unwrap_or_else(|| text(&node.label, small, Some(anchor_width), Style::BOLD));
            centered(
                &mut nodes,
                &label,
                x + ox.unwrap_or(0.0),
                y + oy.unwrap_or(-3.0),
                &anchor_attrs,
            );
        } else {
            let label = text(&node.label, small, None, 0);
            let attrs = &node_attrs;
            let lift = LABEL_OFFSET + if strategy.is_some() { 10.0 } else { 0.0 };
            if ox.is_some() || oy.is_some() {
                starting(
                    &mut nodes,
                    &label,
                    x + ox.unwrap_or(lift),
                    y + oy.unwrap_or(-lift),
                    attrs,
                );
            } else if in_pipeline {
                centered(
                    &mut nodes,
                    &label,
                    x,
                    y + 2.0 * R + 4.0 + label.height / 2.0,
                    attrs,
                );
            } else {
                // The label goes up and to the right unless a link leaves that
                // way; then the next free corner.
                let busy = taken.get(&index).copied().unwrap_or(0);
                let corner = if busy & 1 == 0 {
                    1
                } else if busy & 2 == 0 {
                    2
                } else if busy & 4 == 0 {
                    4
                } else if busy & 8 == 0 {
                    8
                } else {
                    1
                };
                let right = corner == 1 || corner == 2;
                let up = corner == 1 || corner == 4;
                starting(
                    &mut nodes,
                    &label,
                    if right {
                        x + lift
                    } else {
                        x - lift - label.width
                    },
                    if up {
                        y - lift
                    } else {
                        y + lift + small * 0.75
                    },
                    attrs,
                );
            }
        }
        grow(x - 2.0 * R, y - 2.0 * R, x + 2.0 * R + 20.0, y + 2.0 * R);
        nodes.push_str("</g>");
    }

    let mut annotations = String::new();
    let mark_attrs = format!(" font-size=\"{}\" font-weight=\"var(--_hw)\"", num(tiny));
    for annotation in &built.annotations {
        let label = text(
            &js::num_str(annotation.number),
            tiny,
            Some(4000.0),
            Style::BOLD,
        );
        let _ = write!(
            annotations,
            "<g class=\"{p}-wardley-annotation\" data-id=\"{}\">",
            EscText(&js::num_str(annotation.number))
        );
        for coordinate in &annotation.coordinates {
            let x = project_x(coordinate.x);
            let y = project_y(coordinate.y);
            let r = 10f64.max(label.width / 2.0 + 4.0);
            grow(x - r, y - r, x + r, y + r);
            let _ = write!(
                annotations,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_bg)\" stroke=\"{LINE}\"/>{}",
                num(x),
                num(y),
                num(r),
                label.svg(Point::new(x, y), &mark_attrs)
            );
        }
        annotations.push_str("</g>");
    }
    let mut listed: Vec<_> = built
        .annotations
        .iter()
        .filter(|a| a.text.as_deref().is_some_and(|t| !t.is_empty()))
        .collect();
    listed.sort_by(|a, b| js::compare(a.number - b.number));
    if !listed.is_empty() {
        let list_width = if tight {
            80f64.max(plot_w - 20.0)
        } else {
            320.0
        };
        let lines: Vec<Label> = listed
            .iter()
            .map(|a| {
                text(
                    &format!(
                        "{}. {}",
                        js::Number(a.number),
                        a.text.as_deref().unwrap_or("")
                    ),
                    small,
                    Some(list_width),
                    0,
                )
            })
            .collect();
        let mut width = 0f64;
        let mut height = 0.0;
        for line in &lines {
            width = width.max(line.width);
            height += line.height;
        }
        width += 20.0;
        height += 14.0;
        // Without a position the list goes under the map, where it covers
        // nothing.
        let (bx, by) = match built.annotations_box {
            Some(b) => (
                clamp(project_x(b.x), 0.0, 0f64.max(plot_w - width)),
                clamp(project_y(b.y), 0.0, 0f64.max(plot_h - height)),
            ),
            None => (0.0, legend_top),
        };
        grow(bx, by, bx + width, by + height);
        let _ = write!(
            annotations,
            "<g class=\"{p}-wardley-annotations\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"/>",
            num(bx),
            num(by),
            num(width),
            num(height)
        );
        let row_attrs = format!(" font-size=\"{}\"", num(small));
        let mut y = by + 7.0;
        for line in &lines {
            let _ = write!(
                annotations,
                "{}",
                StartLabel(
                    line,
                    bx + 10.0,
                    y + line.line_height / 2.0 + small * 0.35,
                    &row_attrs,
                )
            );
            y += line.height;
        }
        annotations.push_str("</g>");
    }

    let mut notes = String::new();
    let note_attrs = format!(
        " class=\"{p}-wardley-note\" font-size=\"{}\" font-weight=\"var(--_hw)\"",
        num(small)
    );
    for note in &built.notes {
        starting(
            &mut notes,
            &text(
                &note.text,
                small,
                Some(360f64.min(plot_w * 0.6)),
                Style::BOLD,
            ),
            project_x(note.x),
            project_y(note.y),
            &note_attrs,
        );
    }

    // Accelerators point right, deaccelerators left.
    let mut forces = String::new();
    let force_attrs = format!(
        " font-size=\"{}\" font-weight=\"var(--_hw)\"",
        num(tiny + 1.0)
    );
    let mut force = |name: &str, px: f64, py: f64, forward: bool| {
        let x = project_x(px);
        let y = project_y(py);
        let half = FORCE_HEIGHT / 2.0;
        let tip = if forward { x + FORCE_WIDTH } else { x };
        let tail = if forward { x } else { x + FORCE_WIDTH };
        let neck = if forward {
            x + FORCE_WIDTH - FORCE_HEAD
        } else {
            x + FORCE_HEAD
        };
        grow(x, y - half - 8.0, x + FORCE_WIDTH, y + half + 8.0);
        let _ = write!(
            forces,
            "<g class=\"{p}-wardley-{}\" data-id=\"{}\"><path d=\"M{}H{}V{}L{}L{}V{}H{}Z\" fill=\"var(--_s)\" stroke=\"{LINE}\"/>",
            if forward {
                "accelerator"
            } else {
                "deaccelerator"
            },
            EscText(name),
            Point::new(tail, y - half),
            num(neck),
            num(y - half - 8.0),
            Point::new(tip, y),
            Point::new(neck, y + half + 8.0),
            num(y + half),
            num(tail)
        );
        centered(
            &mut forces,
            &text(name, tiny + 1.0, Some(160.0), Style::BOLD),
            x + FORCE_WIDTH / 2.0,
            y + half + 8.0 + 12.0,
            &force_attrs,
        );
        forces.push_str("</g>");
    };
    for a in &built.accelerators {
        force(&a.name, a.x, a.y, true);
    }
    for d in &built.deaccelerators {
        force(&d.name, d.x, d.y, false);
    }

    let title = Label::layout(
        model.titles.title.as_deref(),
        false,
        measure,
        size,
        if tight { 80f64.max(plot_w) } else { 4000.0 },
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let (min_x, min_y) = (bounds.min_x.get(), bounds.min_y.get());
    let (max_x, max_y) = (bounds.max_x.get(), bounds.max_y.get());
    let left = min_x.min(plot_w / 2.0 - title.width / 2.0);
    let right = max_x.max(plot_w / 2.0 + title.width / 2.0);
    let tx = pad - left;
    let ty = pad + title_height - min_y;
    let total_width = (right - left + 2.0 * pad).ceil();
    let total_height = (max_y - min_y + title_height + 2.0 * pad).ceil();
    let title_attrs = crate::svg::builder::TITLE;
    let mut content = String::with_capacity(
        frame.len()
            + stage_labels.len()
            + axis_labels.len()
            + pipelines.len()
            + links.len()
            + trends.len()
            + nodes.len()
            + annotations.len()
            + notes.len()
            + forces.len()
            + 512,
    );
    let _ = write!(
        content,
        "{}",
        title.svg(
            Point::new(tx + plot_w / 2.0, pad + title.height / 2.0),
            title_attrs
        )
    );
    let _ = write!(
        content,
        "<g transform=\"translate({},{})\"><g class=\"{p}-wardley-frame\" fill=\"none\">{frame}</g>",
        num(tx),
        num(ty)
    );
    content.push_str(&stage_labels);
    content.push_str(&axis_labels);
    content.push_str(&pipelines);
    if !links.is_empty() {
        let _ = write!(content, "<g class=\"{p}-edges\" fill=\"none\">{links}</g>");
    }
    if !trends.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-wardley-trends\" fill=\"none\">{trends}</g>"
        );
    }
    content.push_str(&nodes);
    content.push_str(&annotations);
    content.push_str(&notes);
    content.push_str(&forces);
    content.push_str("</g>");

    Rendered {
        svg: svg_document(
            "wardley",
            total_width,
            total_height,
            size,
            ctx,
            Accessible {
                acc_title: model.titles.acc_title.as_deref(),
                acc_descr: model.titles.acc_descr.as_deref(),
            },
            &content,
        ),
        width: total_width,
        height: total_height,
        links: Vec::new(),
    }
}
