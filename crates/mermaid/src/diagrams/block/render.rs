//! Pele's `diagrams/block/render.ts`.

use std::{
    collections::{HashMap, HashSet},
    f64::consts::{FRAC_1_SQRT_2, PI},
    fmt::Write as _,
};

use super::{
    arrows::{Axis, OVERHANG, arrow_axis, arrow_mask, arrow_size, draw_arrow},
    layout::{Cell, layout},
    types::{Block, BlockModel},
};
use crate::Ctx;
use crate::{
    Config, Error, Rendered,
    config::Value,
    diagrams::common::fit_width::{title_room, try_tighten},
    js,
    svg::{
        builder::{esc_text, label_svg, num},
        edges::{EdgePath, edge_label_svg, marker, marker_trim},
        root::{Accessible, svg_document},
        shapes::{draw_shape, shape_inset, shape_size},
        theme::{RADIUS, ResolvedStyle, class_names, resolve_style},
    },
    text::{Host, Label, Style, layout_label},
};

const GAP: f64 = 12.0;
const WRAP: f64 = 200.0;
const CORNER: f64 = 8.0;
const SHAPE_ATTRS: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";
const LINE_ATTRS: &str = " stroke=\"var(--_b)\"";
const DEFAULT_CLASS: [&str; 1] = ["default"];
/// Checking every edge against every block is quadratic, so large diagrams
/// route without it.
const OBSTACLE_BUDGET: usize = 2_000_000;
const SHAPES: [(&str, &str); 14] = [
    ("square", "rect"),
    ("round", "rounded"),
    ("circle", "circle"),
    ("doublecircle", "dbl-circ"),
    ("diamond", "diam"),
    ("hexagon", "hex"),
    ("stadium", "stadium"),
    ("subroutine", "fr-rect"),
    ("cylinder", "cyl"),
    ("rect_left_inv_arrow", "odd"),
    ("lean_right", "lean-r"),
    ("lean_left", "lean-l"),
    ("trapezoid", "trap-b"),
    ("inv_trapezoid", "trap-t"),
];
/// Shapes that keep their own size in a larger cell, where the others
/// stretch to fill it.
const FIXED: [&str; 3] = ["circle", "dbl-circ", "diam"];
/// Shapes that only widen, because their slants and curves are set by their
/// height.
const WIDE: [&str; 7] = [
    "stadium", "hex", "odd", "lean-r", "lean-l", "trap-b", "trap-t",
];
const SPOTS: [f64; 5] = [0.5, 0.3, 0.7, 0.15, 0.85];

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewKind {
    Leaf,
    Arrow,
    Composite,
    Space,
}

/// What a block draws as. Its place is the [`Cell`] of the same index.
struct View {
    block: usize,
    kind: ViewKind,
    index: usize,
    shape: &'static str,
    label: Label,
    style: ResolvedStyle,
    mask: u32,
    dx: f64,
    dy: f64,
    /// Size as drawn, which for a fixed shape is smaller than its cell.
    bw: f64,
    bh: f64,
    x: f64,
    y: f64,
}

struct Route {
    edge: usize,
    /// Two points for a straight line, three for a bend, none for a
    /// self-loop.
    points: Vec<f64>,
    loop_k: f64,
    label: Label,
    lx: f64,
    ly: f64,
}

fn is_round(v: &View) -> bool {
    v.kind == ViewKind::Leaf && (v.shape == "circle" || v.shape == "dbl-circ")
}

/// Where a vertical line at x meets the outline, below the center (sign 1)
/// or above it (sign -1).
fn edge_y(v: &View, x: f64, sign: f64) -> f64 {
    let hw = v.bw / 2.0;
    let hh = v.bh / 2.0;
    let dx = (x - v.x).abs();
    let mut reach = hh;
    if is_round(v) {
        reach = (hw * hw - dx * dx).max(0.0).sqrt();
    } else if v.kind == ViewKind::Leaf && v.shape == "diam" {
        reach = hh * (1.0 - dx / hw).max(0.0);
    } else if v.kind == ViewKind::Leaf {
        reach = hh - shape_inset(v.shape, v.bw, v.bh, if sign > 0.0 { 2 } else { 0 }, 0.0);
    } else if v.kind == ViewKind::Arrow && arrow_axis(v.mask) == Axis::X {
        reach = hh - OVERHANG;
    }
    v.y + sign * reach
}

/// Where a horizontal line at y meets the outline, right of the center
/// (sign 1) or left of it (sign -1).
fn edge_x(v: &View, y: f64, sign: f64) -> f64 {
    let hw = v.bw / 2.0;
    let hh = v.bh / 2.0;
    let dy = (y - v.y).abs();
    let mut reach = hw;
    if is_round(v) {
        reach = (hh * hh - dy * dy).max(0.0).sqrt();
    } else if v.kind == ViewKind::Leaf && v.shape == "diam" {
        reach = hw * (1.0 - dy / hh).max(0.0);
    } else if v.kind == ViewKind::Leaf {
        reach = hw - shape_inset(v.shape, v.bw, v.bh, if sign > 0.0 { 1 } else { 3 }, 0.0);
    } else if v.kind == ViewKind::Arrow && arrow_axis(v.mask) == Axis::Y {
        reach = hw - OVERHANG;
    }
    v.x + sign * reach
}

/// `x || 1`.
fn or_one(x: f64) -> f64 {
    if js::truthy(x) { x } else { 1.0 }
}

/// Where the line from the center toward (tx, ty) leaves the outline.
fn border(v: &View, tx: f64, ty: f64, out: &mut Vec<f64>) {
    let dx = tx - v.x;
    let dy = ty - v.y;
    let hw = v.bw / 2.0;
    let hh = v.bh / 2.0;
    let mut k = if is_round(v) {
        hw / or_one(js::hypot(dx, dy))
    } else if v.kind == ViewKind::Leaf && v.shape == "diam" {
        1.0 / or_one(dx.abs() / hw + dy.abs() / hh)
    } else {
        js::min(
            if dx == 0.0 {
                f64::INFINITY
            } else {
                hw / dx.abs()
            },
            if dy == 0.0 {
                f64::INFINITY
            } else {
                hh / dy.abs()
            },
        )
    };
    if k == f64::INFINITY {
        k = 0.0;
    }
    out.push(v.x + dx * k);
    out.push(v.y + dy * k);
}

fn crosses_box(x1: f64, y1: f64, x2: f64, y2: f64, v: &View) -> bool {
    let l = v.x - v.bw / 2.0 + 1.0;
    let r = v.x + v.bw / 2.0 - 1.0;
    let t = v.y - v.bh / 2.0 + 1.0;
    let b = v.y + v.bh / 2.0 - 1.0;
    if (x1 < l && x2 < l) || (x1 > r && x2 > r) || (y1 < t && y2 < t) || (y1 > b && y2 > b) {
        return false;
    }
    if x1 == x2 || y1 == y2 {
        return true;
    }
    // The segment is diagonal and its bounding box overlaps: the box is hit
    // unless all four corners are on one side.
    let dx = x2 - x1;
    let dy = y2 - y1;
    let side = |x: f64, y: f64| js::sign(dx * (y - y1) - dy * (x - x1));
    let first = side(l, t);
    first == 0.0 || side(r, t) != first || side(r, b) != first || side(l, b) != first
}

fn hits(points: &[f64], s: usize, t: usize, views: &[View], obstacles: &[usize]) -> usize {
    let mut count = 0;
    for &o in obstacles {
        if o == s || o == t {
            continue;
        }
        let mut i = 0;
        while i + 3 < points.len() {
            if crosses_box(
                points[i],
                points[i + 1],
                points[i + 2],
                points[i + 3],
                &views[o],
            ) {
                count += 1;
                break;
            }
            i += 2;
        }
    }
    count
}

/// A straight line where the two blocks share a row or a column, otherwise
/// one bend between facing sides.
fn route_edge(
    views: &[View],
    si: usize,
    ti: usize,
    shift: f64,
    obstacles: Option<&[usize]>,
) -> Vec<f64> {
    let (s, t) = (&views[si], &views[ti]);
    let sl = s.x - s.bw / 2.0;
    let sr = s.x + s.bw / 2.0;
    let st = s.y - s.bh / 2.0;
    let sb = s.y + s.bh / 2.0;
    let tl = t.x - t.bw / 2.0;
    let tr = t.x + t.bw / 2.0;
    let tt = t.y - t.bh / 2.0;
    let tb = t.y + t.bh / 2.0;
    let x1 = js::max(sl, tl);
    let x2 = js::min(sr, tr);
    let y1 = js::max(st, tt);
    let y2 = js::min(sb, tb);
    let over_x = x2 - x1 > 6.0;
    let over_y = y2 - y1 > 6.0;

    if over_x && over_y {
        // One block is inside the other: join the inner one to the nearest
        // side of the outer one.
        let inner_is_s = s.bw * s.bh <= t.bw * t.bh;
        let (inner, outer) = if inner_is_s { (s, t) } else { (t, s) };
        let up = inner.y - inner.bh / 2.0 - (outer.y - outer.bh / 2.0);
        let down = outer.y + outer.bh / 2.0 - (inner.y + inner.bh / 2.0);
        let left = inner.x - inner.bw / 2.0 - (outer.x - outer.bw / 2.0);
        let right = outer.x + outer.bw / 2.0 - (inner.x + inner.bw / 2.0);
        let least = js::min(js::min(up, down), js::min(left, right));
        let points = if least == down {
            vec![
                inner.x,
                edge_y(inner, inner.x, 1.0),
                inner.x,
                outer.y + outer.bh / 2.0,
            ]
        } else if least == up {
            vec![
                inner.x,
                edge_y(inner, inner.x, -1.0),
                inner.x,
                outer.y - outer.bh / 2.0,
            ]
        } else if least == right {
            vec![
                edge_x(inner, inner.y, 1.0),
                inner.y,
                outer.x + outer.bw / 2.0,
                inner.y,
            ]
        } else {
            vec![
                edge_x(inner, inner.y, -1.0),
                inner.y,
                outer.x - outer.bw / 2.0,
                inner.y,
            ]
        };
        return if inner_is_s {
            points
        } else {
            vec![points[2], points[3], points[0], points[1]]
        };
    }
    if over_x {
        let margin = js::min(6.0, (x2 - x1) / 2.0);
        let x = js::min(js::max((x1 + x2) / 2.0 + shift, x1 + margin), x2 - margin);
        let down = if t.y > s.y { 1.0 } else { -1.0 };
        return vec![x, edge_y(s, x, down), x, edge_y(t, x, -down)];
    }
    if over_y {
        let margin = js::min(6.0, (y2 - y1) / 2.0);
        let y = js::min(js::max((y1 + y2) / 2.0 + shift, y1 + margin), y2 - margin);
        let right = if t.x > s.x { 1.0 } else { -1.0 };
        return vec![edge_x(s, y, right), y, edge_x(t, y, -right), y];
    }

    let right = if t.x > s.x { 1.0 } else { -1.0 };
    let down = if t.y > s.y { 1.0 } else { -1.0 };
    let across = vec![
        edge_x(s, s.y, right),
        s.y,
        t.x,
        s.y,
        t.x,
        edge_y(t, t.x, -down),
    ];
    let along = vec![
        s.x,
        edge_y(s, s.x, down),
        s.x,
        t.y,
        edge_x(t, t.y, -right),
        t.y,
    ];
    let wide = (t.x - s.x).abs() >= (t.y - s.y).abs();
    let (mut best, other) = if wide {
        (across, along)
    } else {
        (along, across)
    };
    let Some(obstacles) = obstacles else {
        return best;
    };
    let mut least = hits(&best, si, ti, views, obstacles);
    if least == 0 {
        return best;
    }
    let count = hits(&other, si, ti, views, obstacles);
    if count < least {
        best = other;
        least = count;
    }
    if least == 0 {
        return best;
    }
    let mut direct = Vec::new();
    border(s, t.x, t.y, &mut direct);
    border(t, s.x, s.y, &mut direct);
    if hits(&direct, si, ti, views, obstacles) < least {
        direct
    } else {
        best
    }
}

/// Builds the path through two or three points, rounding the bend and
/// leaving room for markers.
fn trace(p: &[f64], mut start_trim: f64, mut end_trim: f64) -> EdgePath {
    let n = p.len();
    let sx = p[0];
    let sy = p[1];
    let ex = p[n - 2];
    let ey = p[n - 1];
    let mut ax = p[2] - sx;
    let mut ay = p[3] - sy;
    let first = js::hypot(ax, ay);
    ax /= or_one(first);
    ay /= or_one(first);
    let mut bx = ex - p[n - 4];
    let mut by = ey - p[n - 3];
    let last = js::hypot(bx, by);
    bx /= or_one(last);
    by /= or_one(last);
    if n == 4 && start_trim + end_trim > first {
        let scale = first / (start_trim + end_trim);
        start_trim *= scale;
        end_trim *= scale;
    }
    start_trim = js::min(start_trim, first);
    end_trim = js::min(end_trim, last);
    let mut d = format!(
        "M{},{}",
        num(sx + ax * start_trim),
        num(sy + ay * start_trim)
    );
    if n == 6 {
        let r = js::max(
            0.0,
            js::min(js::min(CORNER, first - start_trim), last - end_trim),
        );
        let _ = write!(
            d,
            "L{},{}Q{},{} {},{}",
            num(p[2] - ax * r),
            num(p[3] - ay * r),
            num(p[2]),
            num(p[3]),
            num(p[2] + bx * r),
            num(p[3] + by * r)
        );
    }
    let _ = write!(
        d,
        "L{},{}",
        num(ex - bx * end_trim),
        num(ey - by * end_trim)
    );
    EdgePath {
        d,
        sx,
        sy,
        sdx: -ax,
        sdy: -ay,
        ex,
        ey,
        edx: bx,
        edy: by,
    }
}

/// A self-loop is an arc around the top right corner of the block, or around
/// the matching point on a round or diamond outline. Gives the arc's center
/// and radius and the angles where it leaves and returns.
fn loop_arc(v: &View, k: f64) -> [f64; 5] {
    let hw = v.bw / 2.0;
    let hh = v.bh / 2.0;
    let r = js::max(1.0, js::min(js::min(10.0 + 5.0 * (k - 1.0), hw), hh));
    if is_round(v) {
        let px = v.x + hw * FRAC_1_SQRT_2;
        let py = v.y - hw * FRAC_1_SQRT_2;
        let half = 2.0 * js::min(1.0, r / (2.0 * hw)).asin();
        let angle = |at: f64| (v.y - hw * at.sin() - py).atan2(v.x + hw * at.cos() - px);
        return [px, py, r, angle(PI / 4.0 + half), angle(PI / 4.0 - half)];
    }
    if v.kind == ViewKind::Leaf && v.shape == "diam" {
        let slope = hh.atan2(hw);
        return [v.x + hw / 2.0, v.y - hh / 2.0, r, slope + PI, slope];
    }
    [v.x + hw, v.y - hh, r, PI, PI / 2.0]
}

fn loop_path(v: &View, k: f64, start_trim: f64, end_trim: f64) -> EdgePath {
    let [px, py, r, a1, a2] = loop_arc(v, k);
    let from = a1 + start_trim / r;
    let to = a2 - end_trim / r;
    let mut sweep = (to - from) % (2.0 * PI);
    if sweep <= 0.0 {
        sweep += 2.0 * PI;
    }
    let x1 = px + r * from.cos();
    let y1 = py + r * from.sin();
    let x2 = px + r * to.cos();
    let y2 = py + r * to.sin();
    let sx = px + r * a1.cos();
    let sy = py + r * a1.sin();
    let ex = px + r * a2.cos();
    let ey = py + r * a2.sin();
    // Each marker lies along the chord it covers, so it follows the curve
    // instead of the tangent at its tip.
    let sl = js::hypot(sx - x1, sy - y1);
    let el = js::hypot(ex - x2, ey - y2);
    EdgePath {
        d: format!(
            "M{},{}A{},{} 0 {} 1 {},{}",
            num(x1),
            num(y1),
            num(r),
            num(r),
            if sweep > PI { 1 } else { 0 },
            num(x2),
            num(y2)
        ),
        sx,
        sy,
        sdx: if sl > 0.0 { (sx - x1) / sl } else { a1.sin() },
        sdy: if sl > 0.0 { (sy - y1) / sl } else { -a1.cos() },
        ex,
        ey,
        edx: if el > 0.0 { (ex - x2) / el } else { -a2.sin() },
        edy: if el > 0.0 { (ey - y2) / el } else { a2.cos() },
    }
}

/// Picks the point on a leg where a label of the given size covers no block
/// other than the two it joins.
#[allow(clippy::too_many_arguments)]
fn label_spot(
    p: &[f64],
    at: usize,
    w: f64,
    h: f64,
    s: usize,
    t: usize,
    views: &[View],
    obstacles: Option<&[usize]>,
) -> f64 {
    let Some(obstacles) = obstacles else {
        return 0.5;
    };
    for spot in SPOTS {
        let x = p[at] + (p[at + 2] - p[at]) * spot;
        let y = p[at + 1] + (p[at + 3] - p[at + 1]) * spot;
        let clear = obstacles.iter().all(|&o| {
            let v = &views[o];
            o == s
                || o == t
                || !((x - v.x).abs() < (w + v.bw) / 2.0 && (y - v.y).abs() < (h + v.bh) / 2.0)
        });
        if clear {
            return spot;
        }
    }
    0.5
}

pub fn render_block(
    model: &BlockModel,
    config: &Config,
    ctx: &Ctx<'_, impl Host>,
) -> Result<Rendered, Error> {
    try_tighten(ctx, |tight| draw(model, config, ctx, tight))
}

/// The extent of the drawing, grown by each label and loop.
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    fn grow(&mut self, x: f64, y: f64, w: f64, h: f64) {
        if x - w / 2.0 < self.min_x {
            self.min_x = x - w / 2.0;
        }
        if x + w / 2.0 > self.max_x {
            self.max_x = x + w / 2.0;
        }
        if y - h / 2.0 < self.min_y {
            self.min_y = y - h / 2.0;
        }
        if y + h / 2.0 > self.max_y {
            self.max_y = y + h / 2.0;
        }
    }
}

struct Viewer<'a, H> {
    model: &'a BlockModel,
    ctx: &'a Ctx<'a, H>,
    size: f64,
    gap: f64,
    wrap: f64,
    tight: bool,
}

impl<H: Host> Viewer<'_, H> {
    fn style_of(&self, block: &Block) -> ResolvedStyle {
        let mut declarations: Vec<&str> = Vec::new();
        // A style list is split on commas only, so one entry can still hold
        // several declarations.
        fn add<'a>(declarations: &mut Vec<&'a str>, styles: &'a [String]) {
            for style in styles {
                declarations.extend(style.split(';'));
            }
        }
        let classes = self.model.classes_of(block);
        if classes.is_empty() {
            for name in DEFAULT_CLASS {
                if let Some(def) = self.model.classes.get(name) {
                    add(&mut declarations, &def.styles);
                }
            }
        } else {
            for name in classes {
                if let Some(def) = self.model.classes.get(name) {
                    add(&mut declarations, &def.styles);
                }
            }
        }
        if let Some(styles) = &block.styles {
            add(&mut declarations, styles);
        }
        resolve_style(&declarations)
    }

    fn view(&self, at: usize, index: usize) -> (View, Cell) {
        let block = self.model.block(at);
        let kind_str = block.kind.as_deref();
        let style = if kind_str == Some("space") {
            resolve_style::<&str>(&[])
        } else {
            self.style_of(block)
        };
        let text_style = (if style.bold { Style::BOLD } else { 0 })
            | (if style.italic { Style::ITALIC } else { 0 });
        let font_size = style.font_size.unwrap_or(self.size);
        let mut kind = ViewKind::Leaf;
        let mut shape = "";
        let label: Label;
        let mut nw = 0.0;
        let mut nh = 0.0;
        let mut head = 0.0;
        let mut mask = 0;
        let mut dx = 0.0;
        let mut dy = 0.0;
        let raw = block.label.as_deref();
        if kind_str == Some("space") {
            kind = ViewKind::Space;
            label = layout_label(None, false, self.ctx.host, font_size, self.wrap, 0);
        } else if kind_str == Some("composite") {
            kind = ViewKind::Composite;
            let room = if self.tight {
                title_room(self.ctx)
            } else {
                4000.0
            };
            label = layout_label(raw, false, self.ctx.host, font_size, room, text_style);
            head = if label.height > 0.0 {
                label.height + 4.0
            } else {
                0.0
            };
            nw = label.width + 2.0 * self.gap;
            nh = head + 2.0 * self.gap;
        } else if kind_str == Some("block_arrow") {
            kind = ViewKind::Arrow;
            label = layout_label(raw, false, self.ctx.host, font_size, self.wrap, text_style);
            let blank = label
                .lines
                .iter()
                .flatten()
                .all(|part| part.icon.is_none() && js::trim(&part.text).is_empty());
            mask = arrow_mask(block.directions.as_deref());
            let s = if blank {
                arrow_size(mask, 0.0, 0.0)
            } else {
                arrow_size(mask, label.width, label.height)
            };
            nw = s.w;
            nh = s.h;
            dx = s.dx;
            dy = s.dy;
        } else {
            let kind_str = kind_str.unwrap_or("");
            shape = SHAPES
                .iter()
                .find(|(name, _)| *name == kind_str)
                .map_or("rect", |(_, shape)| shape);
            label = layout_label(raw, false, self.ctx.host, font_size, self.wrap, text_style);
            let s = shape_size(shape, label.width, label.height);
            nw = s.w;
            // A cylinder's caps deepen as it widens, so it is sized for the
            // deepest ones.
            nh = if shape == "cyl" {
                s.h.max(label.height + 40.0)
            } else {
                s.h
            };
            dy = s.dy;
        }
        let cell = Cell {
            columns: if kind == ViewKind::Composite {
                block.columns.unwrap_or(-1.0)
            } else {
                -1.0
            },
            span: if kind == ViewKind::Space {
                1.0
            } else {
                block.width_in_columns.unwrap_or(1.0)
            },
            space: kind == ViewKind::Space,
            head,
            nw,
            nh,
            ..Cell::default()
        };
        let view = View {
            block: at,
            kind,
            index,
            shape,
            label,
            style,
            mask,
            dx,
            dy,
            bw: 0.0,
            bh: 0.0,
            x: 0.0,
            y: 0.0,
        };
        (view, cell)
    }
}

fn draw(
    model: &BlockModel,
    config: &Config,
    ctx: &Ctx<'_, impl Host>,
    tight: bool,
) -> Result<Rendered, Error> {
    let p = crate::PREFIX;
    let size = ctx.options.font_size;
    let edge_size = js::round(size * 0.875);
    let measure = ctx.host;
    let pad = ctx.options.padding.unwrap_or(8.0);
    let configured = match config.section("block").and_then(|b| b.get("padding")) {
        Some(Value::Number(n)) if *n >= 0.0 => n.min(200.0),
        _ => GAP,
    };
    let gap = configured.min(if tight { 6.0 } else { f64::INFINITY });
    let viewer = Viewer {
        model,
        ctx,
        size,
        gap,
        wrap: if tight { 80.0 } else { WRAP },
        tight,
    };

    let (mut root, mut root_cell) = viewer.view(model.root, 0);
    root.index = 0;
    root_cell.nw = 0.0;
    root_cell.nh = 0.0;
    let mut views = vec![root];
    let mut cells = vec![root_cell];
    let mut by_id: HashMap<&str, usize> = HashMap::new();
    let mut i = 0;
    while i < views.len() {
        if views[i].kind == ViewKind::Composite {
            let children = model
                .block(views[i].block)
                .children
                .as_deref()
                .unwrap_or_default();
            for &child in children {
                let index = views.len();
                let (v, cell) = viewer.view(child, index);
                cells[i].children.push(index);
                if v.kind != ViewKind::Space {
                    by_id.insert(model.block(child).id.as_deref().unwrap_or(""), index);
                }
                views.push(v);
                cells.push(cell);
            }
        }
        i += 1;
    }

    let (content_width, content_height) = layout(&mut cells, gap)?;

    let mut obstacles: Vec<usize> = Vec::new();
    for (at, (v, c)) in views.iter_mut().zip(&cells).enumerate() {
        v.x = c.x;
        v.y = c.y;
        if v.kind == ViewKind::Leaf && FIXED.contains(&v.shape) {
            v.bw = c.nw;
            v.bh = c.nh;
        } else if v.kind == ViewKind::Arrow {
            v.bw = if c.span > 1.0 { c.w } else { c.nw };
            v.bh = c.nh;
        } else {
            v.bw = c.w;
            v.bh = if v.kind == ViewKind::Leaf && WIDE.contains(&v.shape) {
                c.nh
            } else {
                c.h
            };
        }
        if v.kind == ViewKind::Leaf && v.shape == "cyl" {
            v.dy = 12f64.min(6f64.max(v.bw / 14.0)) / 2.0;
        }
        if v.kind == ViewKind::Leaf || v.kind == ViewKind::Arrow {
            obstacles.push(at);
        }
    }

    // Edges between the same two blocks are spread apart instead of drawn
    // on top of each other.
    let count = views.len();
    let key_of = |s: usize, t: usize| s.min(t) * count + s.max(t);
    let mut pairs: HashMap<usize, usize> = HashMap::new();
    let mut labelled: HashSet<usize> = HashSet::new();
    let mut ends: Vec<(usize, usize)> = Vec::new();
    let mut drawn: Vec<usize> = Vec::new();
    for &edge in &model.edges {
        let block = model.block(edge);
        let s = by_id.get(block.start.as_deref().unwrap_or(""));
        let t = by_id.get(block.end.as_deref().unwrap_or(""));
        let (Some(&s), Some(&t)) = (s, t) else {
            continue;
        };
        let key = key_of(views[s].index, views[t].index);
        *pairs.entry(key).or_insert(0) += 1;
        if block.label.as_deref().is_some_and(|l| !l.is_empty()) {
            labelled.insert(key);
        }
        ends.push((s, t));
        drawn.push(edge);
    }
    let check = (drawn.len() * obstacles.len() <= OBSTACLE_BUDGET).then_some(&obstacles[..]);
    let mut seen: HashMap<usize, usize> = HashMap::new();
    let mut routes: Vec<Route> = Vec::new();
    let mut bounds = Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: content_width,
        max_y: content_height,
    };
    for (i, &edge) in drawn.iter().enumerate() {
        let (s, t) = ends[i];
        let key = key_of(views[s].index, views[t].index);
        let total = pairs.get(&key).copied().unwrap_or(1) as f64;
        let k = seen.get(&key).copied().unwrap_or(0);
        seen.insert(key, k + 1);
        let k = k as f64;
        let label = layout_label(
            model.block(edge).label.as_deref(),
            false,
            measure,
            edge_size,
            WRAP,
            0,
        );
        let points: Vec<f64>;
        let lx: f64;
        let ly: f64;
        if s == t {
            let [px, py, r, _, _] = loop_arc(&views[s], k + 1.0);
            points = Vec::new();
            bounds.grow(px, py, 2.0 * r + 2.0, 2.0 * r + 2.0);
            lx = px + r + 6.0 + label.width / 2.0;
            ly = py - r / 2.0;
        } else {
            let step = if labelled.contains(&key) {
                js::round(edge_size * 1.5) + 2.0
            } else {
                14.0
            };
            points = route_edge(&views, s, t, (k - (total - 1.0) / 2.0) * step, check);
            let n = points.len();
            // The label goes on the longer leg.
            let first_leg = (points[2] - points[0]).abs() + (points[3] - points[1]).abs();
            let last_leg =
                (points[n - 2] - points[n - 4]).abs() + (points[n - 1] - points[n - 3]).abs();
            let at = if n == 6 && last_leg > first_leg { 2 } else { 0 };
            let spot = if label.width > 0.0 {
                label_spot(
                    &points,
                    at,
                    label.width + 8.0,
                    label.height,
                    s,
                    t,
                    &views,
                    check,
                )
            } else {
                0.5
            };
            lx = points[at] + (points[at + 2] - points[at]) * spot;
            ly = points[at + 1] + (points[at + 3] - points[at + 1]) * spot;
        }
        if label.width > 0.0 {
            bounds.grow(lx, ly, label.width + 8.0, label.height);
        }
        routes.push(Route {
            edge,
            points,
            loop_k: if s == t { k + 1.0 } else { 0.0 },
            label,
            lx,
            ly,
        });
    }

    let title = layout_label(
        model.title.as_deref(),
        false,
        measure,
        size,
        if tight { title_room(ctx) } else { 4000.0 },
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let Bounds {
        min_x,
        min_y,
        max_x,
        max_y,
    } = bounds;
    let inner = (max_x - min_x).max(title.width);
    let ox = pad - min_x + (inner - (max_x - min_x)) / 2.0;
    let oy = pad + title_height - min_y;
    for v in &mut views {
        v.x += ox;
        v.y += oy;
    }
    let width = (inner + 2.0 * pad).ceil();
    let height = (max_y - min_y + title_height + 2.0 * pad).ceil();

    let mut clusters = String::new();
    let mut nodes = String::new();
    for (v, c) in views.iter().zip(&cells).skip(1) {
        if v.kind == ViewKind::Space {
            continue;
        }
        let block = model.block(v.block);
        let classes = class_names(&model.classes_of(block).join(" "));
        let id = esc_text(block.id.as_deref().unwrap_or(""));
        if v.kind == ViewKind::Composite {
            let top = v.y - c.h / 2.0;
            let _ = write!(
                clusters,
                "<g class=\"{p}-cluster{classes}\" data-id=\"{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_a)\" fill-opacity=\"0.5\" stroke=\"var(--_b)\"{}/>{}</g>",
                num(v.x - c.w / 2.0),
                num(top),
                num(c.w),
                num(c.h),
                v.style.shape,
                label_svg(
                    &v.label,
                    v.x,
                    top + (c.head + gap) / 2.0 + 2.0,
                    &format!(
                        " class=\"{p}-cluster-label\" fill=\"var(--_m)\"{}",
                        v.style.text
                    ),
                ),
            );
            continue;
        }
        let arrow = v.kind == ViewKind::Arrow;
        let shape = if arrow {
            draw_arrow(
                v.mask,
                v.bw,
                v.bh,
                &format!("{SHAPE_ATTRS}{}", v.style.shape),
            )
        } else {
            draw_shape(
                v.shape,
                v.bw,
                v.bh,
                &format!("{SHAPE_ATTRS}{}", v.style.shape),
                &format!("{LINE_ATTRS}{}", v.style.line),
            )
        };
        let _ = write!(
            nodes,
            "<g class=\"{p}-node {p}-shape-{}{classes}\" data-id=\"{id}\" transform=\"translate({},{})\">{shape}{}</g>",
            if arrow { "block-arrow" } else { v.shape },
            num(v.x),
            num(v.y),
            label_svg(
                &v.label,
                v.dx,
                v.dy,
                &format!(" class=\"{p}-label\"{}", v.style.text),
            ),
        );
    }

    let mut edges = String::new();
    let mut labels = String::new();
    for (i, route) in routes.iter_mut().enumerate() {
        let edge = model.block(route.edge);
        let start_type = edge.arrow_type_start.as_deref().unwrap_or("");
        let end_type = edge.arrow_type_end.as_deref().unwrap_or("");
        let path = if route.loop_k > 0.0 {
            loop_path(
                &views[ends[i].0],
                route.loop_k,
                marker_trim(start_type),
                marker_trim(end_type),
            )
        } else {
            for pair in route.points.chunks_mut(2) {
                pair[0] += ox;
                pair[1] += oy;
            }
            trace(
                &route.points,
                marker_trim(start_type),
                marker_trim(end_type),
            )
        };
        let id = edge.id.as_deref().unwrap_or("");
        let _ = write!(
            edges,
            "<g class=\"{p}-edge\" data-id=\"{}\"><path d=\"{}\"{}{}/>{}{}</g>",
            esc_text(id),
            path.d,
            if edge.thickness.as_deref() == Some("thick") {
                " stroke-width=\"2.5\""
            } else {
                ""
            },
            if edge.pattern.as_deref() == Some("dotted") {
                " stroke-dasharray=\"3 4\""
            } else {
                ""
            },
            marker(
                start_type,
                path.sx,
                path.sy,
                path.sdx,
                path.sdy,
                "var(--_l)"
            ),
            marker(end_type, path.ex, path.ey, path.edx, path.edy, "var(--_l)"),
        );
        if route.label.width > 0.0 {
            labels.push_str(&edge_label_svg(
                id,
                &route.label,
                route.lx + ox,
                route.ly + oy,
                "",
            ));
        }
    }

    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
    );
    if !clusters.is_empty() {
        let _ = write!(content, "<g class=\"{p}-clusters\">{clusters}</g>");
    }
    if !edges.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{edges}</g>"
        );
    }
    let _ = write!(content, "<g class=\"{p}-nodes\">{nodes}</g>");
    if !labels.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-edge-labels\" font-size=\"{}\">{labels}</g>",
            js::num_str(edge_size)
        );
    }
    let svg = svg_document(
        "block",
        width,
        height,
        size,
        ctx,
        Accessible::default(),
        &content,
    );
    Ok(Rendered {
        svg,
        width,
        height,
        links: Vec::new(),
    })
}
