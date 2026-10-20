//! Pele's `diagrams/block/render.ts`.

use crate::config::capped;
use crate::geom::{Axis, Point, Side, Size};
use crate::svg::builder::rewrite;
use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};
use std::{
    collections::{HashMap, HashSet},
    f64::consts::{FRAC_1_SQRT_2, PI},
    fmt::Write as _,
};

use super::{
    arrows::{Heads, OVERHANG},
    layout::{Cell, layout},
    types::{ArrowType, Block, BlockKind, BlockModel, Columns, Pattern, Thickness},
};
use crate::{Config, Ctx};
use crate::{
    Error, Rendered,
    diagrams::Render,
    js,
    svg::{
        builder::{EscText, num},
        edges::{EdgePath, Marker, edge_label_svg},
        root::{Accessible, svg_document},
        shapes::Shape,
        theme::{RADIUS, ResolvedStyle, class_names, resolve_style},
    },
    text::{Host, Label, Style},
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
/// Shapes that keep their own size in a larger cell, where the others
/// stretch to fill it.
const FIXED: [Shape; 3] = [Shape::Circle, Shape::DoubleCircle, Shape::Diamond];
/// Shapes that only widen, because their slants and curves are set by their
/// height.
const WIDE: [Shape; 7] = [
    Shape::Stadium,
    Shape::Hexagon,
    Shape::Odd,
    Shape::LeanRight,
    Shape::LeanLeft,
    Shape::TrapezoidBottom,
    Shape::TrapezoidTop,
];
const SPOTS: [f64; 5] = [0.5, 0.3, 0.7, 0.15, 0.85];

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewKind {
    Leaf(Shape),
    Arrow(Heads),
    Composite,
    Space,
}

/// What a block draws as. Its place is the [`Cell`] of the same index.
struct View {
    block: usize,
    kind: ViewKind,
    index: usize,
    label: Label,
    style: ResolvedStyle,
    /// Shift of the label from the center.
    shift: Point,
    /// Size as drawn, which for a fixed shape is smaller than its cell.
    size: Size,
    center: Point,
}

struct Route {
    edge: usize,
    /// Two points for a straight line, three for a bend, none for a
    /// self-loop.
    points: Vec<Point>,
    loop_k: f64,
    label: Label,
    label_at: Point,
}

/// The arc a self-loop follows: its center and radius, and the angles where
/// it leaves and returns.
struct Arc {
    center: Point,
    r: f64,
    from: f64,
    to: f64,
}

impl View {
    fn is_round(&self) -> bool {
        matches!(
            self.kind,
            ViewKind::Leaf(Shape::Circle | Shape::DoubleCircle)
        )
    }

    fn is_diamond(&self) -> bool {
        self.kind == ViewKind::Leaf(Shape::Diamond)
    }

    /// Where a vertical line at x meets the outline, below the center (sign
    /// 1) or above it (sign -1).
    fn edge_y(&self, x: f64, sign: f64) -> f64 {
        let hw = self.size.w / 2.0;
        let hh = self.size.h / 2.0;
        let dx = (x - self.center.x).abs();
        let mut reach = hh;
        if self.is_round() {
            reach = (hw * hw - dx * dx).max(0.0).sqrt();
        } else if self.is_diamond() {
            reach = hh * (1.0 - dx / hw).max(0.0);
        } else if let ViewKind::Leaf(shape) = self.kind {
            reach = hh
                - shape.inset(
                    self.size,
                    if sign > 0.0 { Side::Bottom } else { Side::Top },
                    0.0,
                );
        } else if let ViewKind::Arrow(heads) = self.kind
            && heads.axis() == Some(Axis::X)
        {
            reach = hh - OVERHANG;
        }
        self.center.y + sign * reach
    }

    /// Where a horizontal line at y meets the outline, right of the center
    /// (sign 1) or left of it (sign -1).
    fn edge_x(&self, y: f64, sign: f64) -> f64 {
        let hw = self.size.w / 2.0;
        let hh = self.size.h / 2.0;
        let dy = (y - self.center.y).abs();
        let mut reach = hw;
        if self.is_round() {
            reach = (hh * hh - dy * dy).max(0.0).sqrt();
        } else if self.is_diamond() {
            reach = hw * (1.0 - dy / hh).max(0.0);
        } else if let ViewKind::Leaf(shape) = self.kind {
            reach = hw
                - shape.inset(
                    self.size,
                    if sign > 0.0 { Side::Right } else { Side::Left },
                    0.0,
                );
        } else if let ViewKind::Arrow(heads) = self.kind
            && heads.axis() == Some(Axis::Y)
        {
            reach = hw - OVERHANG;
        }
        self.center.x + sign * reach
    }

    /// Where the line from the center toward `to` leaves the outline.
    fn border(&self, to: Point) -> Point {
        let dx = to.x - self.center.x;
        let dy = to.y - self.center.y;
        let hw = self.size.w / 2.0;
        let hh = self.size.h / 2.0;
        let mut k = if self.is_round() {
            hw / js::or_one(js::hypot(dx, dy))
        } else if self.is_diamond() {
            1.0 / js::or_one(dx.abs() / hw + dy.abs() / hh)
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
        Point::new(self.center.x + dx * k, self.center.y + dy * k)
    }

    /// Whether the segment from `a` to `b` crosses the box, less a pixel on
    /// each side.
    fn crossed_by(&self, a: Point, b: Point) -> bool {
        let (Point { x: x1, y: y1 }, Point { x: x2, y: y2 }) = (a, b);
        let l = self.center.x - self.size.w / 2.0 + 1.0;
        let r = self.center.x + self.size.w / 2.0 - 1.0;
        let t = self.center.y - self.size.h / 2.0 + 1.0;
        let b = self.center.y + self.size.h / 2.0 - 1.0;
        if (x1 < l && x2 < l) || (x1 > r && x2 > r) || (y1 < t && y2 < t) || (y1 > b && y2 > b) {
            return false;
        }
        if x1 == x2 || y1 == y2 {
            return true;
        }
        // The segment is diagonal and its bounding box overlaps: the box is
        // hit unless all four corners are on one side.
        let dx = x2 - x1;
        let dy = y2 - y1;
        let side = |x: f64, y: f64| js::sign(dx * (y - y1) - dy * (x - x1));
        let first = side(l, t);
        first == 0.0 || side(r, t) != first || side(r, b) != first || side(l, b) != first
    }

    /// The arc of the `k`th self-loop: around the top right corner of the
    /// block, or around the matching point on a round or diamond outline.
    fn loop_arc(&self, k: f64) -> Arc {
        let (Point { x, y }, hw, hh) = (self.center, self.size.w / 2.0, self.size.h / 2.0);
        let r = js::max(1.0, js::min(js::min(10.0 + 5.0 * (k - 1.0), hw), hh));
        if self.is_round() {
            let px = x + hw * FRAC_1_SQRT_2;
            let py = y - hw * FRAC_1_SQRT_2;
            let half = 2.0 * js::min(1.0, r / (2.0 * hw)).asin();
            let angle = |at: f64| (y - hw * at.sin() - py).atan2(x + hw * at.cos() - px);
            return Arc {
                center: Point::new(px, py),
                r,
                from: angle(PI / 4.0 + half),
                to: angle(PI / 4.0 - half),
            };
        }
        if self.is_diamond() {
            let slope = hh.atan2(hw);
            return Arc {
                center: Point::new(x + hw / 2.0, y - hh / 2.0),
                r,
                from: slope + PI,
                to: slope,
            };
        }
        Arc {
            center: Point::new(x + hw, y - hh),
            r,
            from: PI,
            to: PI / 2.0,
        }
    }
}

impl Arc {
    fn path(&self, start_trim: f64, end_trim: f64) -> EdgePath {
        let (Point { x: px, y: py }, r, a1, a2) = (self.center, self.r, self.from, self.to);
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
            start: Point::new(sx, sy),
            start_dir: Point::new(
                if sl > 0.0 { (sx - x1) / sl } else { a1.sin() },
                if sl > 0.0 { (sy - y1) / sl } else { -a1.cos() },
            ),
            end: Point::new(ex, ey),
            end_dir: Point::new(
                if el > 0.0 { (ex - x2) / el } else { -a2.sin() },
                if el > 0.0 { (ey - y2) / el } else { a2.cos() },
            ),
        }
    }
}

/// Routes edges between the laid out blocks, around the blocks in
/// `obstacles` when it is given.
struct Router<'a> {
    views: &'a [View],
    obstacles: Option<&'a [usize]>,
}

impl Router<'_> {
    /// How many obstacles other than `s` and `t` the line through `points`
    /// crosses.
    fn hits(&self, obstacles: &[usize], points: &[Point], s: usize, t: usize) -> usize {
        let mut count = 0;
        for &o in obstacles {
            if o == s || o == t {
                continue;
            }
            if points
                .windows(2)
                .any(|leg| self.views[o].crossed_by(leg[0], leg[1]))
            {
                count += 1;
            }
        }
        count
    }

    /// A straight line where the two blocks share a row or a column,
    /// otherwise one bend between facing sides.
    fn route(&self, si: usize, ti: usize, shift: f64) -> Vec<Point> {
        let views = self.views;
        let (s, t) = (&views[si], &views[ti]);
        let sl = s.center.x - s.size.w / 2.0;
        let sr = s.center.x + s.size.w / 2.0;
        let st = s.center.y - s.size.h / 2.0;
        let sb = s.center.y + s.size.h / 2.0;
        let tl = t.center.x - t.size.w / 2.0;
        let tr = t.center.x + t.size.w / 2.0;
        let tt = t.center.y - t.size.h / 2.0;
        let tb = t.center.y + t.size.h / 2.0;
        let x1 = js::max(sl, tl);
        let x2 = js::min(sr, tr);
        let y1 = js::max(st, tt);
        let y2 = js::min(sb, tb);
        let over_x = x2 - x1 > 6.0;
        let over_y = y2 - y1 > 6.0;

        if over_x && over_y {
            // One block is inside the other: join the inner one to the
            // nearest side of the outer one.
            let inner_is_s = s.size.w * s.size.h <= t.size.w * t.size.h;
            let (inner, outer) = if inner_is_s { (s, t) } else { (t, s) };
            let (i, o) = (inner.center, outer.center);
            let up = i.y - inner.size.h / 2.0 - (o.y - outer.size.h / 2.0);
            let down = o.y + outer.size.h / 2.0 - (i.y + inner.size.h / 2.0);
            let left = i.x - inner.size.w / 2.0 - (o.x - outer.size.w / 2.0);
            let right = o.x + outer.size.w / 2.0 - (i.x + inner.size.w / 2.0);
            let least = js::min(js::min(up, down), js::min(left, right));
            let mut points = if least == down {
                vec![
                    Point::new(i.x, inner.edge_y(i.x, 1.0)),
                    Point::new(i.x, o.y + outer.size.h / 2.0),
                ]
            } else if least == up {
                vec![
                    Point::new(i.x, inner.edge_y(i.x, -1.0)),
                    Point::new(i.x, o.y - outer.size.h / 2.0),
                ]
            } else if least == right {
                vec![
                    Point::new(inner.edge_x(i.y, 1.0), i.y),
                    Point::new(o.x + outer.size.w / 2.0, i.y),
                ]
            } else {
                vec![
                    Point::new(inner.edge_x(i.y, -1.0), i.y),
                    Point::new(o.x - outer.size.w / 2.0, i.y),
                ]
            };
            if !inner_is_s {
                points.reverse();
            }
            return points;
        }
        if over_x {
            let margin = js::min(6.0, (x2 - x1) / 2.0);
            let x = js::min(js::max((x1 + x2) / 2.0 + shift, x1 + margin), x2 - margin);
            let down = if t.center.y > s.center.y { 1.0 } else { -1.0 };
            return vec![
                Point::new(x, s.edge_y(x, down)),
                Point::new(x, t.edge_y(x, -down)),
            ];
        }
        if over_y {
            let margin = js::min(6.0, (y2 - y1) / 2.0);
            let y = js::min(js::max((y1 + y2) / 2.0 + shift, y1 + margin), y2 - margin);
            let right = if t.center.x > s.center.x { 1.0 } else { -1.0 };
            return vec![
                Point::new(s.edge_x(y, right), y),
                Point::new(t.edge_x(y, -right), y),
            ];
        }

        let (sc, tc) = (s.center, t.center);
        let right = if tc.x > sc.x { 1.0 } else { -1.0 };
        let down = if tc.y > sc.y { 1.0 } else { -1.0 };
        let across = vec![
            Point::new(s.edge_x(sc.y, right), sc.y),
            Point::new(tc.x, sc.y),
            Point::new(tc.x, t.edge_y(tc.x, -down)),
        ];
        let along = vec![
            Point::new(sc.x, s.edge_y(sc.x, down)),
            Point::new(sc.x, tc.y),
            Point::new(t.edge_x(tc.y, -right), tc.y),
        ];
        let wide = (tc.x - sc.x).abs() >= (tc.y - sc.y).abs();
        let (mut best, other) = if wide {
            (across, along)
        } else {
            (along, across)
        };
        let Some(obstacles) = self.obstacles else {
            return best;
        };
        let mut least = self.hits(obstacles, &best, si, ti);
        if least == 0 {
            return best;
        }
        let count = self.hits(obstacles, &other, si, ti);
        if count < least {
            best = other;
            least = count;
        }
        if least == 0 {
            return best;
        }
        let direct = vec![s.border(tc), t.border(sc)];
        if self.hits(obstacles, &direct, si, ti) < least {
            direct
        } else {
            best
        }
    }

    /// Picks the point on the leg from `p[at]` where a label of `size` covers
    /// no block other than the two it joins.
    fn label_spot(&self, p: &[Point], at: usize, size: Size, s: usize, t: usize) -> f64 {
        let Some(obstacles) = self.obstacles else {
            return 0.5;
        };
        for spot in SPOTS {
            let x = p[at].x + (p[at + 1].x - p[at].x) * spot;
            let y = p[at].y + (p[at + 1].y - p[at].y) * spot;
            let clear = obstacles.iter().all(|&o| {
                let v = &self.views[o];
                o == s
                    || o == t
                    || !((x - v.center.x).abs() < (size.w + v.size.w) / 2.0
                        && (y - v.center.y).abs() < (size.h + v.size.h) / 2.0)
            });
            if clear {
                return spot;
            }
        }
        0.5
    }
}

/// Builds the path through two or three points, rounding the bend and
/// leaving room for markers.
fn trace(p: &[Point], mut start_trim: f64, mut end_trim: f64) -> EdgePath {
    let n = p.len();
    let Point { x: sx, y: sy } = p[0];
    let Point { x: ex, y: ey } = p[n - 1];
    let mut ax = p[1].x - sx;
    let mut ay = p[1].y - sy;
    let first = js::hypot(ax, ay);
    ax /= js::or_one(first);
    ay /= js::or_one(first);
    let mut bx = ex - p[n - 2].x;
    let mut by = ey - p[n - 2].y;
    let last = js::hypot(bx, by);
    bx /= js::or_one(last);
    by /= js::or_one(last);
    if n == 2 && start_trim + end_trim > first {
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
    if n == 3 {
        let r = js::max(
            0.0,
            js::min(js::min(CORNER, first - start_trim), last - end_trim),
        );
        let bend = p[1];
        let _ = write!(
            d,
            "L{},{}Q{},{} {},{}",
            num(bend.x - ax * r),
            num(bend.y - ay * r),
            num(bend.x),
            num(bend.y),
            num(bend.x + bx * r),
            num(bend.y + by * r)
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
        start: Point::new(sx, sy),
        start_dir: Point::new(-ax, -ay),
        end: Point::new(ex, ey),
        end_dir: Point::new(bx, by),
    }
}

/// Mermaid's `block` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BlockConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub padding: Option<f64>,
}

impl Render for BlockModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        ctx.try_tighten(|tight| self.draw(&config.block, ctx, tight))
    }
}

/// The extent of the drawing, grown by each label and loop.
struct Bounds {
    min: Point,
    max: Point,
}

impl Bounds {
    /// Grows the extent to hold a box of `size` centered on `center`.
    fn grow(&mut self, center: Point, size: Size) {
        let Point { x, y } = center;
        let Size { w, h } = size;
        if x - w / 2.0 < self.min.x {
            self.min.x = x - w / 2.0;
        }
        if x + w / 2.0 > self.max.x {
            self.max.x = x + w / 2.0;
        }
        if y - h / 2.0 < self.min.y {
            self.min.y = y - h / 2.0;
        }
        if y + h / 2.0 > self.max.y {
            self.max.y = y + h / 2.0;
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
        let style = if block.kind == BlockKind::Space {
            resolve_style::<&str>(&[])
        } else {
            self.style_of(block)
        };
        let text_style = (if style.bold { Style::BOLD } else { 0 })
            | (if style.italic { Style::ITALIC } else { 0 });
        let font_size = style.font_size.unwrap_or(self.size);
        let kind;
        let label: Label;
        let mut need = Size::default();
        let mut head = 0.0;
        let mut shift = Point::default();
        let raw = block.label.as_deref();
        match block.kind {
            BlockKind::Space => {
                kind = ViewKind::Space;
                label = Label::layout(None, false, self.ctx.host, font_size, self.wrap, 0);
            }
            BlockKind::Composite => {
                kind = ViewKind::Composite;
                let room = if self.tight {
                    self.ctx.title_room()
                } else {
                    4000.0
                };
                label = Label::layout(raw, false, self.ctx.host, font_size, room, text_style);
                head = label.spaced_height(4.0);
                need = Size::new(label.width + 2.0 * self.gap, head + 2.0 * self.gap);
            }
            BlockKind::BlockArrow => {
                let heads = Heads::from_directions(block.directions.as_deref());
                kind = ViewKind::Arrow(heads);
                label = Label::layout(raw, false, self.ctx.host, font_size, self.wrap, text_style);
                let blank = label
                    .lines
                    .iter()
                    .flatten()
                    .all(|part| part.icon.is_none() && js::trim(&part.text).is_empty());
                let s = heads.size(if blank {
                    Size::default()
                } else {
                    Size::new(label.width, label.height)
                });
                need = s.size;
                shift = s.shift;
            }
            other => {
                let shape = other.shape();
                kind = ViewKind::Leaf(shape);
                label = Label::layout(raw, false, self.ctx.host, font_size, self.wrap, text_style);
                let s = shape.size(Size::new(label.width, label.height));
                // A cylinder's caps deepen as it widens, so it is sized for
                // the deepest ones.
                need = Size::new(
                    s.size.w,
                    if shape == Shape::Cylinder {
                        s.size.h.max(label.height + 40.0)
                    } else {
                        s.size.h
                    },
                );
                shift.y = s.dy;
            }
        }
        let cell = Cell {
            columns: if kind == ViewKind::Composite {
                block.columns.unwrap_or(Columns::Auto)
            } else {
                Columns::Auto
            },
            span: if kind == ViewKind::Space {
                1.0
            } else {
                block.width_in_columns.unwrap_or(1.0)
            },
            space: kind == ViewKind::Space,
            head,
            need,
            ..Cell::default()
        };
        let view = View {
            block: at,
            kind,
            index,
            label,
            style,
            shift,
            size: Size::default(),
            center: Point::default(),
        };
        (view, cell)
    }
}

impl BlockModel {
    fn draw(
        &self,
        config: &BlockConfig,
        ctx: &Ctx<'_, impl Host>,
        tight: bool,
    ) -> Result<Rendered, Error> {
        let p = crate::PREFIX;
        let model = self;
        let size = ctx.options.font_size;
        let edge_size = (size * 0.875).round();
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);
        let configured = capped(config.padding, GAP, 0.0, 200.0);
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
        root_cell.need = Size::default();
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

        let content = layout(&mut cells, gap)?;

        let mut obstacles: Vec<usize> = Vec::new();
        for (at, (v, c)) in views.iter_mut().zip(&cells).enumerate() {
            v.center = c.center;
            v.size = match v.kind {
                ViewKind::Leaf(shape) if FIXED.contains(&shape) => c.need,
                ViewKind::Arrow(_) => {
                    Size::new(if c.span > 1.0 { c.size.w } else { c.need.w }, c.need.h)
                }
                ViewKind::Leaf(shape) if WIDE.contains(&shape) => Size::new(c.size.w, c.need.h),
                _ => c.size,
            };
            if v.kind == ViewKind::Leaf(Shape::Cylinder) {
                v.shift.y = 12f64.min(6f64.max(v.size.w / 14.0)) / 2.0;
            }
            if matches!(v.kind, ViewKind::Leaf(_) | ViewKind::Arrow(_)) {
                obstacles.push(at);
            }
        }

        // Edges between the same two blocks are spread apart instead of
        // drawn on top of each other.
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
        let router = Router {
            views: &views,
            obstacles: (drawn.len() * obstacles.len() <= OBSTACLE_BUDGET).then_some(&obstacles[..]),
        };
        let mut seen: HashMap<usize, usize> = HashMap::new();
        let mut routes: Vec<Route> = Vec::new();
        let mut bounds = Bounds {
            min: Point::default(),
            max: Point::new(content.w, content.h),
        };
        for (i, &edge) in drawn.iter().enumerate() {
            let (s, t) = ends[i];
            let key = key_of(views[s].index, views[t].index);
            let total = pairs.get(&key).copied().unwrap_or(1) as f64;
            let k = seen.get(&key).copied().unwrap_or(0);
            seen.insert(key, k + 1);
            let k = k as f64;
            let label = Label::layout(
                model.block(edge).label.as_deref(),
                false,
                measure,
                edge_size,
                WRAP,
                0,
            );
            let (points, label_at) = if s == t {
                let Arc { center, r, .. } = views[s].loop_arc(k + 1.0);
                bounds.grow(center, Size::new(2.0 * r + 2.0, 2.0 * r + 2.0));
                (
                    Vec::new(),
                    Point::new(center.x + r + 6.0 + label.width / 2.0, center.y - r / 2.0),
                )
            } else {
                let step = if labelled.contains(&key) {
                    (edge_size * 1.5).round() + 2.0
                } else {
                    14.0
                };
                let points = router.route(s, t, (k - (total - 1.0) / 2.0) * step);
                let n = points.len();
                let leg = |at: usize| {
                    (points[at + 1].x - points[at].x).abs()
                        + (points[at + 1].y - points[at].y).abs()
                };
                // The label goes on the longer leg.
                let at = if n == 3 && leg(1) > leg(0) { 1 } else { 0 };
                let spot = if label.width > 0.0 {
                    router.label_spot(
                        &points,
                        at,
                        Size::new(label.width + 8.0, label.height),
                        s,
                        t,
                    )
                } else {
                    0.5
                };
                let label_at = points[at] + (points[at + 1] - points[at]) * spot;
                (points, label_at)
            };
            if label.width > 0.0 {
                bounds.grow(label_at, Size::new(label.width + 8.0, label.height));
            }
            routes.push(Route {
                edge,
                points,
                loop_k: if s == t { k + 1.0 } else { 0.0 },
                label,
                label_at,
            });
        }

        let title = Label::layout(
            model.title.as_deref(),
            false,
            measure,
            size,
            if tight { ctx.title_room() } else { 4000.0 },
            Style::BOLD,
        );
        let title_height = title.spaced_height(12.0);
        let Bounds { min, max } = bounds;
        let inner = (max.x - min.x).max(title.width);
        let offset = Point::new(
            pad - min.x + (inner - (max.x - min.x)) / 2.0,
            pad + title_height - min.y,
        );
        for v in &mut views {
            v.center += offset;
        }
        let width = (inner + 2.0 * pad).ceil();
        let height = (max.y - min.y + title_height + 2.0 * pad).ceil();

        let mut clusters = String::new();
        let mut nodes = String::new();
        let mut shape_attrs_buf = String::new();
        let mut label_attrs_buf = String::new();
        let mut line_attrs = String::new();
        let mut title_attrs = String::new();
        for (v, c) in views.iter().zip(&cells).skip(1) {
            let shape = match v.kind {
                ViewKind::Space => continue,
                ViewKind::Leaf(shape) => Some(shape),
                ViewKind::Arrow(_) | ViewKind::Composite => None,
            };
            let block = model.block(v.block);
            let classes = class_names(&model.classes_of(block).join(" "));
            let id = EscText(block.id.as_deref().unwrap_or(""));
            if v.kind == ViewKind::Composite {
                let top = v.center.y - c.size.h / 2.0;
                let _ = write!(
                    clusters,
                    "<g class=\"{p}-cluster{classes}\" data-id=\"{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_a)\" fill-opacity=\"0.5\" stroke=\"var(--_b)\"{}/>{}</g>",
                    num(v.center.x - c.size.w / 2.0),
                    num(top),
                    num(c.size.w),
                    num(c.size.h),
                    v.style.shape,
                    v.label.svg(
                        Point::new(v.center.x, top + (c.head + gap) / 2.0 + 2.0),
                        rewrite(
                            &mut title_attrs,
                            format_args!(
                                " class=\"{p}-cluster-label\" fill=\"var(--_m)\"{}",
                                v.style.text
                            )
                        )
                    ),
                );
                continue;
            }
            let _ = write!(
                nodes,
                "<g class=\"{p}-node {p}-shape-{}{classes}\" data-id=\"{id}\" transform=\"translate({},{})\">",
                shape.map_or("block-arrow", Shape::name),
                num(v.center.x),
                num(v.center.y)
            );
            let shape_attrs = rewrite(
                &mut shape_attrs_buf,
                format_args!("{SHAPE_ATTRS}{}", v.style.shape),
            );
            match v.kind {
                ViewKind::Arrow(heads) => heads.draw(&mut nodes, v.size, shape_attrs),
                ViewKind::Leaf(shape) => shape.draw(
                    &mut nodes,
                    v.size,
                    shape_attrs,
                    rewrite(
                        &mut line_attrs,
                        format_args!("{LINE_ATTRS}{}", v.style.line),
                    ),
                ),
                ViewKind::Composite | ViewKind::Space => {}
            }
            let label_attrs = rewrite(
                &mut label_attrs_buf,
                format_args!(" class=\"{p}-label\"{}", v.style.text),
            );
            let _ = write!(nodes, "{}</g>", v.label.svg(v.shift, label_attrs));
        }

        let mut edges = String::new();
        let mut labels = String::new();
        for (i, route) in routes.iter_mut().enumerate() {
            let edge = model.block(route.edge);
            let line = edge.line;
            let start = line.and_then(|line| line.arrow_start.marker());
            let end = line.and_then(|line| line.arrow_end.and_then(ArrowType::marker));
            let trims = (Marker::trim_of(start), Marker::trim_of(end));
            let path = if route.loop_k > 0.0 {
                views[ends[i].0]
                    .loop_arc(route.loop_k)
                    .path(trims.0, trims.1)
            } else {
                for point in &mut route.points {
                    *point += offset;
                }
                trace(&route.points, trims.0, trims.1)
            };
            let id = edge.id.as_deref().unwrap_or("");
            let _ = write!(
                edges,
                "<g class=\"{p}-edge\" data-id=\"{}\"><path d=\"{}\"{}{}/>{}{}</g>",
                EscText(id),
                path.d,
                if line.is_some_and(|line| line.thickness == Thickness::Thick) {
                    " stroke-width=\"2.5\""
                } else {
                    ""
                },
                if line.is_some_and(|line| line.pattern == Pattern::Dotted) {
                    " stroke-dasharray=\"3 4\""
                } else {
                    ""
                },
                Marker::draw_of(start, path.start, path.start_dir, "var(--_l)"),
                Marker::draw_of(end, path.end, path.end_dir, "var(--_l)"),
            );
            if route.label.width > 0.0 {
                let _ = write!(
                    labels,
                    "{}",
                    edge_label_svg(id, &route.label, route.label_at + offset, "")
                );
            }
        }

        let title_attrs = crate::svg::builder::TITLE;
        let mut content =
            String::with_capacity(clusters.len() + edges.len() + nodes.len() + labels.len() + 512);
        let _ = write!(
            content,
            "{}",
            title.svg(
                Point::new(width / 2.0, pad + title.height / 2.0),
                title_attrs
            )
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
                js::Number(edge_size)
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
}
