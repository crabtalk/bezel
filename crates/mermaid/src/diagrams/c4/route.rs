//! Pele's `diagrams/c4/route.ts`.

use std::{collections::HashMap, fmt::Write as _};

use super::types::C4Ref;
use crate::{
    geom::{Point, Side, Size},
    js::{self, max, min, sign},
    svg::{builder::num, edges::EdgePath, shapes::Shape},
};

const LOOP: f64 = 28.0;
pub const LANE: f64 = 20.0;
const LANE_STEP: f64 = 7.0;
const CORNER: f64 = 8.0;
/// A wide label on an upright lane leans this far out from it, to the middle
/// of the gap between columns.
const LEAN: f64 = 30.0;
/// Label widths that fit the gap between two columns, and where there is
/// room to spare.
const NARROW: f64 = 88.0;
const WIDE: f64 = 160.0;
/// The shared person shape: a head of radius 13 centered 14 below the top,
/// over a body that starts 26 below it.
pub const HEAD: f64 = 26.0;
/// Parallel stretches closer than this read as one line, and a line this
/// close to a boundary's frame reads as part of it.
const NEAR: f64 = 6.0;
const FRAME_NEAR: f64 = 3.0;
const PORT_STEP: f64 = 12.0;

/// A laid out shape or boundary: top-left corner, size, the shared shape it
/// is drawn as, and which it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C4Box {
    pub at: Point,
    pub size: Size,
    pub form: Shape,
    pub id: C4Ref,
}

impl C4Box {
    pub fn right(&self) -> f64 {
        self.at.x + self.size.w
    }

    pub fn bottom(&self) -> f64 {
        self.at.y + self.size.h
    }

    /// The bottom-right corner.
    pub fn far(&self) -> Point {
        Point::new(self.right(), self.bottom())
    }

    pub fn center(&self) -> Point {
        Point::new(self.at.x + self.size.w / 2.0, self.at.y + self.size.h / 2.0)
    }

    fn contains(&self, inner: &C4Box) -> bool {
        self.at.x <= inner.at.x
            && self.at.y <= inner.at.y
            && self.right() >= inner.right()
            && self.bottom() >= inner.bottom()
    }

    /// Where the segment from `p` to `q`, from outside, first meets the
    /// outline of the shape, as a fraction of the segment.
    fn hit(&self, p: Point, q: Point) -> f64 {
        let (Point { x, y }, Size { w, h }) = (self.at, self.size);
        let d = q - p;
        let rect = |x0: f64, y0: f64, x1: f64, y1: f64| {
            hit_rect(Point::new(x0, y0), Point::new(x1, y1), p, d)
        };
        let ellipse =
            |cx: f64, cy: f64, rx: f64, ry: f64| hit_ellipse(Point::new(cx, cy), rx, ry, p, d);
        match self.form {
            Shape::Person => min(
                rect(x, y + HEAD, x + w, y + h),
                ellipse(x + w / 2.0, y + 14.0, 13.0, 13.0),
            ),
            Shape::Cylinder => {
                let ry = cyl_ry(w);
                min(
                    min(
                        rect(x, y + ry, x + w, y + h - ry),
                        ellipse(x + w / 2.0, y + ry, w / 2.0, ry),
                    ),
                    ellipse(x + w / 2.0, y + h - ry, w / 2.0, ry),
                )
            }
            Shape::HorizontalCylinder => {
                let rx = queue_rx(h);
                min(
                    min(
                        rect(x + rx, y, x + w - rx, y + h),
                        ellipse(x + rx, y + h / 2.0, rx, h / 2.0),
                    ),
                    ellipse(x + w - rx, y + h / 2.0, rx, h / 2.0),
                )
            }
            _ => rect(x, y, x + w, y + h),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Route {
    /// The line, and where its markers go.
    pub path: EdgePath,
    /// The stretch the label may sit on.
    pub along: (Point, Point),
    /// The side of that stretch the label is pushed to, as a unit vector;
    /// zero centers it on the stretch.
    pub normal: Point,
    /// When set, the label stays over the line and moves that way by at most
    /// this much.
    pub lean: f64,
    /// The widest the label should be.
    pub room: f64,
    /// The corners of the line.
    pub points: Vec<Point>,
}

/// The cap radii `Shape::draw` uses for the two cylinders.
pub fn cyl_ry(w: f64) -> f64 {
    min(12.0, max(6.0, w / 14.0))
}

pub fn queue_rx(h: f64) -> f64 {
    min(12.0, h / 4.0)
}

/// How far along the segment from `p` by `d` it first meets the rectangle
/// from `lo` to `hi`; infinity when it misses.
fn hit_rect(lo: Point, hi: Point, p: Point, d: Point) -> f64 {
    let mut enter = 0.0;
    let mut leave = 1.0;
    if d.x == 0.0 {
        if p.x < lo.x || p.x > hi.x {
            return f64::INFINITY;
        }
    } else {
        let a = (lo.x - p.x) / d.x;
        let b = (hi.x - p.x) / d.x;
        enter = max(enter, min(a, b));
        leave = min(leave, max(a, b));
    }
    if d.y == 0.0 {
        if p.y < lo.y || p.y > hi.y {
            return f64::INFINITY;
        }
    } else {
        let a = (lo.y - p.y) / d.y;
        let b = (hi.y - p.y) / d.y;
        enter = max(enter, min(a, b));
        leave = min(leave, max(a, b));
    }
    if enter <= leave { enter } else { f64::INFINITY }
}

fn hit_ellipse(c: Point, rx: f64, ry: f64, p: Point, d: Point) -> f64 {
    let ux = d.x / rx;
    let uy = d.y / ry;
    let vx = (p.x - c.x) / rx;
    let vy = (p.y - c.y) / ry;
    let a = ux * ux + uy * uy;
    let b = 2.0 * (ux * vx + uy * vy);
    let c = vx * vx + vy * vy - 1.0;
    let disc = b * b - 4.0 * a * c;
    if a == 0.0 || disc < 0.0 {
        return f64::INFINITY;
    }
    let t = (-b - disc.sqrt()) / (2.0 * a);
    if (0.0..=1.0).contains(&t) {
        t
    } else if c <= 0.0 {
        0.0
    } else {
        f64::INFINITY
    }
}

/// Whether the line through `points` meets the rectangle from `lo` to `hi`.
pub fn crosses(points: &[Point], lo: Point, hi: Point) -> bool {
    points
        .windows(2)
        .any(|leg| hit_rect(lo, hi, leg[0], leg[1] - leg[0]) <= 1.0)
}

/// The straight line from a to b: its start, then its end.
fn connect(a: &C4Box, b: &C4Box, shift: f64) -> (Point, Point) {
    let Point { x: ax, y: ay } = a.center();
    let Point { x: bx, y: by } = b.center();
    let down = a.contains(b);
    if down || b.contains(a) {
        // One holds the other: a short line from the nearest side of the
        // outer box.
        let (outer, inner) = if down { (a, b) } else { (b, a) };
        let Point { x: ix, y: iy } = inner.center();
        let left = inner.at.x - outer.at.x;
        let right = outer.right() - inner.at.x - inner.size.w;
        let top = inner.at.y - outer.at.y;
        let bottom = outer.bottom() - inner.at.y - inner.size.h;
        let least = min(min(left, right), min(top, bottom));
        let ox = if least == left {
            outer.at.x
        } else if least == right {
            outer.right()
        } else {
            ix
        };
        let oy = if least == left || least == right {
            iy
        } else if least == bottom {
            outer.bottom()
        } else {
            outer.at.y
        };
        let t = min(inner.hit(Point::new(ox, oy), Point::new(ix, iy)), 1.0);
        let o = Point::new(ox, oy);
        let m = Point::new(ox + (ix - ox) * t, oy + (iy - oy) * t);
        return if down { (o, m) } else { (m, o) };
    }

    let mut px = ax;
    let mut py = ay;
    let mut qx = bx;
    let mut qy = by;
    // Shapes that share a column or a row are joined by an upright or level
    // line through the shared span.
    let x0 = max(a.at.x, b.at.x);
    let x1 = min(a.right(), b.right());
    let y0 = max(a.at.y, b.at.y);
    let y1 = min(a.bottom(), b.bottom());
    if x0 < x1 {
        px = (x0 + x1) / 2.0;
        qx = px;
    } else if y0 < y1 {
        py = (y0 + y1) / 2.0;
        qy = py;
    }
    let len = js::hypot(qx - px, qy - py);
    let len = js::or_one(len);
    let nx = (-(qy - py) / len) * shift;
    let ny = ((qx - px) / len) * shift;
    let mut ta = a.hit(Point::new(qx + nx, qy + ny), Point::new(px + nx, py + ny));
    let mut tb = b.hit(Point::new(px + nx, py + ny), Point::new(qx + nx, qy + ny));
    if ta <= 1.0 && tb <= 1.0 {
        px += nx;
        py += ny;
        qx += nx;
        qy += ny;
    } else {
        px = ax;
        py = ay;
        qx = bx;
        qy = by;
        ta = min(a.hit(Point::new(qx, qy), Point::new(px, py)), 1.0);
        tb = min(b.hit(Point::new(px, py), Point::new(qx, qy)), 1.0);
    }
    (
        Point::new(qx + (px - qx) * ta, qy + (py - qy) * ta),
        Point::new(px + (qx - px) * tb, py + (qy - py) * tb),
    )
}

/// An upright stretch at `x = at`, or a level one at `y = at`, from `lo` to
/// `hi` along it.
#[derive(Clone, Copy)]
struct Stretch {
    at: f64,
    lo: f64,
    hi: f64,
}

impl Stretch {
    fn new(at: f64, lo: f64, hi: f64) -> Self {
        Self { at, lo, hi }
    }
}

/// What the routes drawn so far take up, so that the next keeps apart from
/// them: upright and level stretches, boundary frames, and where lines leave
/// or reach each side of a shape.
#[derive(Default)]
pub struct Lanes {
    upright: Vec<Stretch>,
    level: Vec<Stretch>,
    upright_frames: Vec<Stretch>,
    level_frames: Vec<Stretch>,
    taken: HashMap<C4Ref, Vec<(Side, f64)>>,
    /// Routes that found no lane of their own: a line left straight behind a
    /// shape, or a lane that runs along another.
    pub squeezed: usize,
}

impl Lanes {
    pub fn free(&self, upright: bool, at: f64, a0: f64, a1: f64) -> bool {
        let lo = min(a0, a1);
        let hi = max(a0, a1);
        let near = |list: &[Stretch], gap: f64| -> bool {
            list.iter()
                .any(|s| (s.at - at).abs() < gap && s.lo < hi && s.hi > lo)
        };
        let (lines, frames) = if upright {
            (&self.upright, &self.upright_frames)
        } else {
            (&self.level, &self.level_frames)
        };
        !near(lines, NEAR) && !near(frames, FRAME_NEAR)
    }

    pub fn crossings(&self, points: &[Point]) -> usize {
        let mut count = 0;
        for leg in points.windows(2) {
            let (p, q) = (leg[0], leg[1]);
            let upright = q.x == p.x;
            let at = if upright { q.x } else { q.y };
            let (u, v) = if upright { (p.y, q.y) } else { (p.x, q.x) };
            let lo = min(u, v);
            let hi = max(u, v);
            let list = if upright { &self.level } else { &self.upright };
            for s in list {
                if s.at > lo && s.at < hi && s.lo < at && s.hi > at {
                    count += 1;
                }
            }
        }
        count
    }

    pub fn frame(&mut self, b: &C4Box) {
        let (Point { x, y }, Size { w, h }) = (b.at, b.size);
        self.upright_frames
            .extend([Stretch::new(x, y, y + h), Stretch::new(x + w, y, y + h)]);
        self.level_frames
            .extend([Stretch::new(y, x, x + w), Stretch::new(y + h, x, x + w)]);
    }

    pub fn take(&mut self, points: &[Point]) {
        for leg in points.windows(2) {
            let (p, q) = (leg[0], leg[1]);
            if q.x == p.x {
                self.upright
                    .push(Stretch::new(q.x, min(p.y, q.y), max(p.y, q.y)));
            } else if q.y == p.y {
                self.level
                    .push(Stretch::new(q.y, min(p.x, q.x), max(p.x, q.x)));
            }
        }
    }

    /// Where on `side` of a shape a line can leave or arrive, between lo and
    /// hi: the places clear of the lines already there, nearest `want`
    /// first.
    pub fn ports(&self, b: &C4Box, side: Side, want: f64, lo: f64, hi: f64) -> Vec<f64> {
        let used = self.taken.get(&b.id).map_or(&[][..], Vec::as_slice);
        let mut out = Vec::new();
        let mut k = 0u32;
        while k < 9 && out.len() < 4 {
            let at =
                want + (if k & 1 == 1 { 1.0 } else { -1.0 }) * f64::from(k.div_ceil(2)) * PORT_STEP;
            k += 1;
            if at < lo || at > hi {
                continue;
            }
            let clear = !used
                .iter()
                .any(|&(s, u)| s == side && (u - at).abs() < PORT_STEP);
            if clear {
                out.push(at);
            }
        }
        if out.is_empty() { vec![want] } else { out }
    }

    pub fn claim(&mut self, b: &C4Box, side: Side, at: f64) {
        self.taken.entry(b.id).or_default().push((side, at));
    }
}

fn path(points: &[Point], start_trim: f64, end_trim: f64) -> String {
    let n = points.len();
    let first = points[0];
    let ux = sign(points[1].x - first.x);
    let uy = sign(points[1].y - first.y);
    let mut d = format!(
        "M{},{}",
        num(first.x + ux * start_trim),
        num(first.y + uy * start_trim)
    );
    for i in 1..n - 1 {
        let Point { x, y } = points[i];
        let (prev, next) = (points[i - 1], points[i + 1]);
        let ix = sign(x - prev.x);
        let iy = sign(y - prev.y);
        let ox = sign(next.x - x);
        let oy = sign(next.y - y);
        let r = min(
            min(CORNER, (x - prev.x + y - prev.y).abs() / 2.0),
            (next.x - x + next.y - y).abs() / 2.0,
        );
        if ix != 0.0 {
            let _ = write!(d, "H{}", num(x - ix * r));
        } else {
            let _ = write!(d, "V{}", num(y - iy * r));
        }
        let _ = write!(
            d,
            "Q{},{} {},{}",
            num(x),
            num(y),
            num(x + ox * r),
            num(y + oy * r)
        );
    }
    let (last, before) = (points[n - 1], points[n - 2]);
    let lx = sign(last.x - before.x);
    let ly = sign(last.y - before.y);
    if lx != 0.0 {
        d + &format!("H{}", num(last.x - lx * end_trim))
    } else {
        d + &format!("V{}", num(last.y - ly * end_trim))
    }
}

fn clear_of(points: &[Point], others: &[C4Box], a: &C4Box, b: &C4Box) -> bool {
    !others
        .iter()
        .any(|other| other.id != a.id && other.id != b.id && crosses(points, other.at, other.far()))
}

/// Where a bent route's label goes: Pele's partial `Route` over the
/// defaults.
struct Place {
    nx: f64,
    lean: f64,
    room: f64,
}

/// The route for a line with corners through `points`, from a's outline to
/// b's. The label sits on the stretch that starts at `points[along]`.
fn bent(points: Vec<Point>, along: usize, start_trim: f64, end_trim: f64, label: Place) -> Route {
    let n = points.len();
    let (first, last) = (points[0], points[n - 1]);
    Route {
        path: EdgePath {
            d: path(&points, start_trim, end_trim),
            start: first,
            start_dir: Point::new(-sign(points[1].x - first.x), -sign(points[1].y - first.y)),
            end: last,
            end_dir: Point::new(
                sign(last.x - points[n - 2].x),
                sign(last.y - points[n - 2].y),
            ),
        },
        along: (points[along], points[along + 1]),
        normal: Point::new(label.nx, 0.0),
        lean: label.lean,
        room: label.room,
        points,
    }
}

fn across(lo: f64, hi: f64) -> Vec<f64> {
    let mut out = Vec::new();
    let mid = (lo + hi) / 2.0;
    let step = max(LANE_STEP, (hi - lo) / 4.0);
    for k in 0u32..9 {
        let at = mid + (if k & 1 == 1 { 1.0 } else { -1.0 }) * f64::from(k.div_ceil(2)) * step;
        if at >= lo + NEAR && at <= hi - NEAR {
            out.push(at);
        }
    }
    out
}

struct Best {
    points: Vec<Point>,
    cost: f64,
    upright: bool,
    sides: [Side; 2],
}

fn elbow(
    a: &C4Box,
    b: &C4Box,
    others: &[C4Box],
    lanes: &mut Lanes,
    start_trim: f64,
    end_trim: f64,
) -> Option<Route> {
    let Point { x: ax, y: ay } = a.center();
    let Point { x: bx, y: by } = b.center();
    let mut best: Option<Best> = None;
    // True when `points` became the best.
    let consider = |best: &mut Option<Best>,
                    lanes: &Lanes,
                    points: Vec<Point>,
                    upright: bool,
                    sides: [Side; 2]|
     -> bool {
        if !clear_of(&points, others, a, b) {
            return false;
        }
        for leg in points.windows(2) {
            let (p, q) = (leg[0], leg[1]);
            let up = q.x == p.x;
            let free = if up {
                lanes.free(up, q.x, p.y, q.y)
            } else {
                lanes.free(up, q.y, p.x, q.x)
            };
            if !free {
                return false;
            }
        }
        let mut cost = lanes.crossings(&points) as f64 * 1e6;
        for leg in points.windows(2) {
            cost += (leg[1].x - leg[0].x).abs() + (leg[1].y - leg[0].y).abs();
        }
        if best.as_ref().is_none_or(|best| cost < best.cost) {
            *best = Some(Best {
                points,
                cost,
                upright,
                sides,
            });
            return true;
        }
        false
    };
    if a.right() <= b.at.x || b.right() <= a.at.x {
        let right = bx > ax;
        let down = sign(by - ay);
        let sides = if right {
            [Side::Right, Side::Left]
        } else {
            [Side::Left, Side::Right]
        };
        let lanes_across = if right {
            across(a.right(), b.at.x)
        } else {
            across(b.right(), a.at.x)
        };
        'search: for y0 in lanes.ports(
            a,
            sides[0],
            ay + (down * a.size.h) / 4.0,
            a.at.y + 8.0,
            a.bottom() - 8.0,
        ) {
            for y1 in lanes.ports(
                b,
                sides[1],
                by - (down * b.size.h) / 4.0,
                b.at.y + 8.0,
                b.bottom() - 8.0,
            ) {
                for &lane in &lanes_across {
                    let x0 = lane
                        + (ax - lane) * min(a.hit(Point::new(lane, y0), Point::new(ax, y0)), 1.0);
                    let x1 = lane
                        + (bx - lane) * min(b.hit(Point::new(lane, y1), Point::new(bx, y1)), 1.0);
                    if consider(
                        &mut best,
                        lanes,
                        vec![
                            Point::new(x0, y0),
                            Point::new(lane, y0),
                            Point::new(lane, y1),
                            Point::new(x1, y1),
                        ],
                        true,
                        sides,
                    ) {
                        break 'search;
                    }
                }
            }
        }
    }
    if a.bottom() <= b.at.y || b.bottom() <= a.at.y {
        let below = by > ay;
        let sideways = sign(bx - ax);
        let sides = if below {
            [Side::Bottom, Side::Top]
        } else {
            [Side::Top, Side::Bottom]
        };
        let lanes_across = if below {
            across(a.bottom(), b.at.y)
        } else {
            across(b.bottom(), a.at.y)
        };
        'search: for x0 in lanes.ports(
            a,
            sides[0],
            ax + (sideways * a.size.w) / 4.0,
            a.at.x + 8.0,
            a.right() - 8.0,
        ) {
            for x1 in lanes.ports(
                b,
                sides[1],
                bx - (sideways * b.size.w) / 4.0,
                b.at.x + 8.0,
                b.right() - 8.0,
            ) {
                for &lane in &lanes_across {
                    let y0 = lane
                        + (ay - lane) * min(a.hit(Point::new(x0, lane), Point::new(x0, ay)), 1.0);
                    let y1 = lane
                        + (by - lane) * min(b.hit(Point::new(x1, lane), Point::new(x1, by)), 1.0);
                    if consider(
                        &mut best,
                        lanes,
                        vec![
                            Point::new(x0, y0),
                            Point::new(x0, lane),
                            Point::new(x1, lane),
                            Point::new(x1, y1),
                        ],
                        false,
                        sides,
                    ) {
                        break 'search;
                    }
                }
            }
        }
    }
    let Best {
        points,
        upright,
        sides,
        ..
    } = best?;
    lanes.take(&points);
    let (first, last) = (points[0], points[3]);
    lanes.claim(a, sides[0], if upright { first.y } else { first.x });
    lanes.claim(b, sides[1], if upright { last.y } else { last.x });
    // The label sits on the middle stretch, across the gap.
    Some(bent(
        points,
        1,
        start_trim,
        end_trim,
        Place {
            nx: 0.0,
            lean: 0.0,
            room: if upright { NARROW } else { WIDE },
        },
    ))
}

/// One side's detour: its corners, the side of the shapes it leaves and
/// reaches, where on them, and how many lanes out it went (5 when none was
/// free).
struct Way {
    points: Vec<Point>,
    side: Side,
    p0: f64,
    p1: f64,
    step: usize,
}

/// Routes a relation from a to b. `shift` moves the line sideways when
/// another runs the opposite way. `others` are the shapes to steer clear of;
/// `None` skips that check. `lanes` holds what the routes before it take up,
/// so that this one keeps apart from them.
pub fn route(
    a: &C4Box,
    b: &C4Box,
    shift: f64,
    others: Option<&[C4Box]>,
    lanes: &mut Lanes,
    start_trim: f64,
    end_trim: f64,
) -> Route {
    if a.id == b.id {
        let spread = min(a.size.h / 2.0 - 4.0, 10.0);
        let len = js::hypot(LOOP, spread);
        let x = a.right();
        let cy = a.at.y + a.size.h / 2.0;
        let ux = LOOP / len;
        let uy = spread / len;
        let label = Point::new(x + LOOP + 4.0, cy);
        return Route {
            path: EdgePath {
                d: format!(
                    "M{},{}C{},{} {},{} {},{}",
                    num(x + ux * start_trim),
                    num(cy - spread - uy * start_trim),
                    num(x + LOOP),
                    num(cy - spread * 2.0),
                    num(x + LOOP),
                    num(cy + spread * 2.0),
                    num(x + ux * end_trim),
                    num(cy + spread + uy * end_trim)
                ),
                start: Point::new(x, cy - spread),
                start_dir: Point::new(-ux, uy),
                end: Point::new(x, cy + spread),
                end_dir: Point::new(-ux, -uy),
            },
            along: (label, label),
            normal: Point::new(1.0, 0.0),
            lean: 0.0,
            room: 64.0,
            points: Vec::new(),
        };
    }

    let (s, e) = connect(a, b, shift);
    let upright = s.x == e.x;
    let in_way = |other: &C4Box| {
        other.id != a.id && other.id != b.id && hit_rect(other.at, other.far(), s, e - s) <= 1.0
    };
    let mut blocked = false;
    if let Some(others) = others
        && !a.contains(b)
        && !b.contains(a)
    {
        for other in others {
            if in_way(other) {
                blocked = true;
            }
        }
    }
    if let Some(others) = others
        && blocked
        && !upright
        && s.y != e.y
    {
        if let Some(around) = elbow(a, b, others, lanes, start_trim, end_trim) {
            return around;
        }
        lanes.squeezed += 1;
    }
    if let Some(others) = others
        && blocked
        && (upright || s.y == e.y)
    {
        let lane_on = |forward: bool| -> f64 {
            let mut lane = if forward {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
            let mut widen = |it: &C4Box| {
                let edge = if upright { it.at.x } else { it.at.y };
                lane = if forward {
                    max(lane, edge + if upright { it.size.w } else { it.size.h })
                } else {
                    min(lane, edge)
                };
            };
            widen(a);
            widen(b);
            for other in others {
                if in_way(other) {
                    widen(other);
                }
            }
            lane
        };
        let lo = if upright {
            min(a.at.y, b.at.y)
        } else {
            min(a.at.x, b.at.x)
        };
        let hi = if upright {
            max(a.bottom(), b.bottom())
        } else {
            max(a.right(), b.right())
        };
        let outside = |forward: bool, lane: f64| -> bool {
            for other in others {
                if other.id == a.id || other.id == b.id {
                    continue;
                }
                let near = if upright {
                    other.at.y < hi && other.bottom() > lo
                } else {
                    other.at.x < hi && other.right() > lo
                };
                let start = if upright { other.at.x } else { other.at.y };
                let end = start + if upright { other.size.w } else { other.size.h };
                if near && (if forward { end > lane } else { start < lane }) {
                    return false;
                }
            }
            true
        };
        let Point { x: ax, y: ay } = a.center();
        let Point { x: bx, y: by } = b.center();
        let towards = if upright {
            sign(by - ay)
        } else {
            sign(bx - ax)
        };
        // The route on one side: each end leaves its shape a quarter of the
        // way from the middle towards the other, clear of the straight lines
        // that meet the middle of that side and of other routes there. The
        // lane runs a little further out for each route already along that
        // gap, and never behind a shape.
        let along = |lanes: &Lanes, forward: bool| -> Way {
            let lane = lane_on(forward);
            let sign = if forward { 1.0 } else { -1.0 };
            let side = match (upright, forward) {
                (true, true) => Side::Right,
                (true, false) => Side::Left,
                (false, true) => Side::Bottom,
                (false, false) => Side::Top,
            };
            let p0 = if upright {
                lanes.ports(
                    a,
                    side,
                    ay + (towards * a.size.h) / 4.0,
                    a.at.y + 8.0,
                    a.bottom() - 8.0,
                )[0]
            } else {
                lanes.ports(
                    a,
                    side,
                    ax + (towards * a.size.w) / 4.0,
                    a.at.x + 8.0,
                    a.right() - 8.0,
                )[0]
            };
            let p1 = if upright {
                lanes.ports(
                    b,
                    side,
                    by - (towards * b.size.h) / 4.0,
                    b.at.y + 8.0,
                    b.bottom() - 8.0,
                )[0]
            } else {
                lanes.ports(
                    b,
                    side,
                    bx - (towards * b.size.w) / 4.0,
                    b.at.x + 8.0,
                    b.right() - 8.0,
                )[0]
            };
            let points_at = |at: f64| -> Vec<Point> {
                if upright {
                    let x0 =
                        at + (ax - at) * min(a.hit(Point::new(at, p0), Point::new(ax, p0)), 1.0);
                    let x1 =
                        at + (bx - at) * min(b.hit(Point::new(at, p1), Point::new(bx, p1)), 1.0);
                    return vec![
                        Point::new(x0, p0),
                        Point::new(at, p0),
                        Point::new(at, p1),
                        Point::new(x1, p1),
                    ];
                }
                let y0 = at + (ay - at) * min(a.hit(Point::new(p0, at), Point::new(p0, ay)), 1.0);
                let y1 = at + (by - at) * min(b.hit(Point::new(p1, at), Point::new(p1, by)), 1.0);
                vec![
                    Point::new(p0, y0),
                    Point::new(p0, at),
                    Point::new(p1, at),
                    Point::new(p1, y1),
                ]
            };
            let mut points = points_at(lane + sign * LANE);
            for k in 0..=4 {
                let at = lane + sign * (LANE + k as f64 * LANE_STEP);
                let tried = points_at(at);
                if !clear_of(&tried, others, a, b) {
                    continue;
                }
                points = tried;
                if lanes.free(upright, at, p0, p1) {
                    return Way {
                        points,
                        side,
                        p0,
                        p1,
                        step: k,
                    };
                }
            }
            Way {
                points,
                side,
                p0,
                p1,
                step: 5,
            }
        };
        // A detour takes the outside of the drawing when only one side is
        // outside, leaving the gaps between columns and rows to the
        // relations that cross them. Otherwise it takes the side where it
        // crosses fewer lines, then the one where it runs nearer its shapes,
        // so that detours spread over both sides.
        let natural = if upright { e.y > s.y } else { e.x < s.x };
        let out = outside(natural, lane_on(natural));
        let back = outside(!natural, lane_on(!natural));
        let first = along(lanes, natural);
        let other = if out == back {
            Some(along(lanes, !natural))
        } else {
            None
        };
        let cost = |way: &Way| lanes.crossings(&way.points) * 8 + way.step;
        let forward = if (back && !out)
            || other
                .as_ref()
                .is_some_and(|other| cost(other) < cost(&first))
        {
            !natural
        } else {
            natural
        };
        let Way {
            points,
            side,
            p0,
            p1,
            step,
        } = if forward == natural {
            first
        } else {
            other.unwrap_or_else(|| along(lanes, !natural))
        };
        let sign = if forward { 1.0 } else { -1.0 };
        if step > 4 {
            lanes.squeezed += 1;
        }
        lanes.take(&points);
        lanes.claim(a, side, p0);
        lanes.claim(b, side, p1);
        return bent(
            points,
            1,
            start_trim,
            end_trim,
            if upright {
                Place {
                    nx: sign,
                    lean: LEAN,
                    room: NARROW,
                }
            } else {
                Place {
                    nx: 0.0,
                    lean: 0.0,
                    room: WIDE,
                }
            },
        );
    }

    let len = js::hypot(e.x - s.x, e.y - s.y);
    let len = js::or_one(len);
    let dx = (e.x - s.x) / len;
    let dy = (e.y - s.y) / len;
    if upright || s.y == e.y {
        lanes.take(&[s, e]);
    }
    Route {
        path: EdgePath {
            d: format!(
                "M{},{}L{},{}",
                num(s.x + dx * start_trim),
                num(s.y + dy * start_trim),
                num(e.x - dx * end_trim),
                num(e.y - dy * end_trim)
            ),
            start: s,
            start_dir: Point::new(-dx, -dy),
            end: e,
            end_dir: Point::new(dx, dy),
        },
        along: (s, e),
        normal: if shift != 0.0 {
            Point::new(-dy, dx)
        } else {
            Point::default()
        },
        lean: 0.0,
        room: if dx.abs() > dy.abs() { NARROW } else { WIDE },
        points: vec![s, e],
    }
}
