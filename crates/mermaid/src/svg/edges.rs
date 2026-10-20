//! Edges: the path of a route, its markers, its label, and loops. Pele's
//! `svg/edges.ts`.

use std::{
    collections::HashMap,
    fmt::{self, Write as _},
};

use super::builder::{EscText, fragment, num};
use crate::{
    geom::{Axis, Point, RoutePoint, Side, Size},
    js,
    layout::{
        compound::{CEdge, CNode},
        layered::ease,
    },
    text::Label,
};

pub const ARROW: f64 = 8.0;
/// How far an edge runs straight before a marker at its end, where it has
/// the room.
pub const LEAD: f64 = 6.0;

/// The most a corner is rounded by in a stepped route.
const CORNER: f64 = 12.0;
/// How far the handles of a wide turn reach along its two legs: a little
/// more than a circle's, which makes the turn fuller.
const HANDLE: f64 = 0.62;

/// A marker at the end of an edge, by Mermaid's names: `arrow_point`,
/// `arrow_circle`, `arrow_cross`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    Arrow,
    Circle,
    Cross,
}

impl Marker {
    /// The marker Mermaid's `name` is, or `None` for one that draws nothing.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "arrow_point" => Some(Marker::Arrow),
            "arrow_circle" => Some(Marker::Circle),
            "arrow_cross" => Some(Marker::Cross),
            _ => None,
        }
    }

    /// How far short of its end an edge stops for the marker.
    pub fn trim(self) -> f64 {
        match self {
            Marker::Arrow => ARROW - 1.0,
            Marker::Circle => ARROW,
            Marker::Cross => 4.0,
        }
    }

    /// [`Marker::trim`] of a marker that may be none.
    pub fn trim_of(marker: Option<Self>) -> f64 {
        marker.map_or(0.0, Self::trim)
    }

    /// The marker with its tip at `tip`, pointing along the unit vector
    /// `dir`.
    pub fn draw(self, tip: Point, dir: Point, color: &str) -> impl fmt::Display + '_ {
        fragment(move |f| {
            let p = crate::PREFIX;
            let n = Point::new(-dir.y, dir.x);
            match self {
                Marker::Arrow => {
                    let b = tip - dir * ARROW;
                    let half = ARROW * 0.42;
                    write!(
                        f,
                        "<path class=\"{p}-marker\" d=\"M{}L{}L{}Z\" fill=\"{color}\" stroke=\"none\"/>",
                        tip,
                        b + n * half,
                        b - n * half
                    )
                }
                Marker::Circle => {
                    let c = tip - dir * 4.0;
                    write!(
                        f,
                        "<circle class=\"{p}-marker\" cx=\"{}\" cy=\"{}\" r=\"3.5\" fill=\"{color}\" stroke=\"none\"/>",
                        num(c.x),
                        num(c.y)
                    )
                }
                Marker::Cross => {
                    let c = tip - dir * 4.0;
                    let a = 3.5;
                    let u = (dir + n) * a;
                    let v = (dir - n) * a;
                    write!(
                        f,
                        "<path class=\"{p}-marker\" d=\"M{}L{}M{}L{}\"/>",
                        c - u,
                        c + u,
                        c - v,
                        c + v
                    )
                }
            }
        })
    }

    /// [`Marker::draw`] of a marker that may be none, which draws nothing.
    pub fn draw_of(
        marker: Option<Self>,
        tip: Point,
        dir: Point,
        color: &str,
    ) -> impl fmt::Display + '_ {
        fragment(move |f| match marker {
            Some(marker) => write!(f, "{}", marker.draw(tip, dir, color)),
            None => Ok(()),
        })
    }

    /// Mermaid's name for the marker, which [`Marker::from_name`] reads.
    pub fn name(self) -> &'static str {
        match self {
            Marker::Arrow => "arrow_point",
            Marker::Circle => "arrow_circle",
            Marker::Cross => "arrow_cross",
        }
    }
}

/// How a route is drawn between its points, by Mermaid's curve names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Curve {
    /// Every curve name but these.
    #[default]
    Smooth,
    Linear,
    Step,
    StepBefore,
    StepAfter,
}

impl Curve {
    pub fn from_name(name: &str) -> Self {
        match name {
            "linear" => Curve::Linear,
            "step" => Curve::Step,
            "stepBefore" => Curve::StepBefore,
            "stepAfter" => Curve::StepAfter,
            _ => Curve::Smooth,
        }
    }

    fn is_stepped(self) -> bool {
        matches!(self, Curve::Step | Curve::StepBefore | Curve::StepAfter)
    }
}

/// An edge's path, and where it ends: `start_dir` and `end_dir` are unit
/// vectors pointing into the node at each end.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EdgePath {
    pub d: String,
    pub start: Point,
    pub start_dir: Point,
    pub end: Point,
    pub end_dir: Point,
}

/// How far from each side of each node the edges that meet it start to
/// turn: as far as the largest marker there needs, for every edge on the
/// side, so none turns sooner than its neighbour and crosses it.
#[derive(Default)]
pub struct SideReach {
    reach: HashMap<(usize, Side), f64>,
}

impl SideReach {
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes a node, the side of it, and the marker of an end there.
    pub fn add(&mut self, node: usize, side: Option<Side>, marker: Option<Marker>) {
        let trim = Marker::trim_of(marker);
        let Some(side) = side else {
            return;
        };
        let key = (node, side);
        if trim > 0.0 && trim + LEAD > self.reach.get(&key).copied().unwrap_or(0.0) {
            self.reach.insert(key, trim + LEAD);
        }
    }

    pub fn of(&self, node: usize, side: Option<Side>) -> f64 {
        side.and_then(|side| self.reach.get(&(node, side)).copied())
            .unwrap_or(0.0)
    }
}

/// The path for a route, trimmed at both ends for markers. Between two
/// points not in line, a route is one S that leaves and arrives along the
/// flow, or a step in a stepped route. Where the route turns a corner, a
/// stepped route rounds it a little and any other but a linear one turns as
/// widely as it can: the turn takes the whole of a leg that ends at a node
/// and half of a leg it shares with the next turn. `start_reach` and
/// `end_reach` say how far from its nodes the edge starts to turn, where
/// other edges on the same side of a node need it to; by default, far enough
/// to clear its own marker.
pub fn route_path(
    route: &[RoutePoint],
    curve: Curve,
    start_trim: f64,
    end_trim: f64,
    start_reach: Option<f64>,
    end_reach: Option<f64>,
) -> EdgePath {
    let start_reach = start_reach.unwrap_or(if start_trim > 0.0 {
        start_trim + LEAD
    } else {
        0.0
    });
    let end_reach = end_reach.unwrap_or(if end_trim > 0.0 { end_trim + LEAD } else { 0.0 });
    let linear = curve == Curve::Linear;
    let stepped = curve.is_stepped();
    let mut pts: Vec<RoutePoint> = Vec::new();
    for &RoutePoint { at: p, axis } in route {
        // A stepped route turns at right angles where a smooth one would
        // curve.
        if stepped
            && let Some(prev) = pts.last().map(|q| q.at)
            && (p.x - prev.x).abs() >= 0.01
            && (p.y - prev.y).abs() >= 0.01
        {
            let my = match curve {
                Curve::StepBefore => prev.y,
                Curve::StepAfter => p.y,
                _ => (prev.y + p.y) / 2.0,
            };
            let mx = match curve {
                Curve::StepBefore => prev.x,
                Curve::StepAfter => p.x,
                _ => (prev.x + p.x) / 2.0,
            };
            let turns = if axis == Axis::Y {
                [Point::new(prev.x, my), Point::new(p.x, my)]
            } else {
                [Point::new(mx, prev.y), Point::new(mx, p.y)]
            };
            for turn in turns {
                if turn.near(prev) || turn.near(p) {
                    continue;
                }
                pts.push(RoutePoint::new(turn, axis));
            }
        }
        pts.push(RoutePoint::new(p, axis));
    }
    let mut last = pts.len() - 1;

    let aligned = |pts: &[RoutePoint], i: usize| {
        (pts[i].at.x - pts[i - 1].at.x).abs() < 0.01 || (pts[i].at.y - pts[i - 1].at.y).abs() < 0.01
    };
    let straight = |pts: &[RoutePoint], i: usize| linear || aligned(pts, i);
    // The way the route heads from point i to point j: along the flow where
    // an S joins them.
    let unit = |pts: &[RoutePoint], i: usize, j: usize, axis: Axis| -> Point {
        let (a, b) = (pts[i].at, pts[j].at);
        if !straight(pts, i.max(j)) {
            return if axis == Axis::Y {
                Point::new(0.0, js::or_one(js::sign(b.y - a.y)))
            } else {
                Point::new(js::or_one(js::sign(b.x - a.x)), 0.0)
            };
        }
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = js::or_one(js::hypot(dx, dy));
        Point::new(dx / len, dy / len)
    };
    let start_dir = unit(&pts, 0, 1, pts[1].axis);
    let end_dir = unit(&pts, last - 1, last, pts[last].axis);
    let start = pts[0].at;
    let end = pts[last].at;
    pts[0].at += start_dir * start_trim;
    pts[last].at = pts[last].at - end_dir * end_trim;

    let length = |pts: &[RoutePoint], i: usize| {
        (pts[i].at.x - pts[i - 1].at.x).abs() + (pts[i].at.y - pts[i - 1].at.y).abs()
    };
    // Whether the route turns a corner at point i, from running one way to
    // running the other.
    let turns = |pts: &[RoutePoint], i: usize, last: usize| {
        !linear
            && i > 0
            && i < last
            && aligned(pts, i)
            && aligned(pts, i + 1)
            && length(pts, i) >= 0.01
            && length(pts, i + 1) >= 0.01
            && ((pts[i].at.x - pts[i - 1].at.x).abs() < 0.01)
                != ((pts[i + 1].at.x - pts[i].at.x).abs() < 0.01)
    };
    // An edge that curves into a marker runs straight for a little way
    // first, where it has the room, so the marker does not sit on the bend.
    let along = |pts: &[RoutePoint], i: usize, u: Point| {
        (pts[i].at.x - pts[i - 1].at.x).abs() * u.x.abs()
            + (pts[i].at.y - pts[i - 1].at.y).abs() * u.y.abs()
    };
    // Of the way from the end of a run to where it turns, the part that
    // makes up for a larger marker beside this one is kept where it can be,
    // and the rest is at most a quarter of what is left, so ends on one side
    // of a node turn equally far from it.
    let lead = |reach: f64, trim: f64, run: f64| {
        let room = 0f64.max(reach - LEAD - trim).min(run / 4.0);
        room + (reach - trim - room).min((run - room) / 4.0)
    };
    let arrives = if end_reach > end_trim && (!straight(&pts, last) || turns(&pts, last - 1, last))
    {
        lead(end_reach, end_trim, along(&pts, last, end_dir))
    } else {
        0.0
    };
    let leaves = if start_reach > start_trim && (!straight(&pts, 1) || turns(&pts, 1, last)) {
        lead(start_reach, start_trim, along(&pts, 1, start_dir))
    } else {
        0.0
    };
    if arrives >= 1.0 {
        let point = RoutePoint::new(pts[last].at - end_dir * arrives, pts[last].axis);
        pts.insert(last, point);
        last += 1;
    }
    if leaves >= 1.0 {
        let point = RoutePoint::new(pts[0].at + start_dir * leaves, pts[1].axis);
        pts.insert(1, point);
        last += 1;
    }

    let mut d = String::new();
    let _ = write!(d, "M{}", pts[0].at);
    let mut at = pts[0].at;
    let line_to = |d: &mut String, at: &mut Point, p: Point| {
        if p.near(*at) {
            return;
        }
        let _ = write!(d, "L{p}");
        *at = p;
    };
    for i in 1..=last {
        let p = pts[i].at;
        if !straight(&pts, i) {
            let down = pts[i].axis == Axis::Y;
            let along = if down { p.y - at.y } else { p.x - at.x };
            let k = along.abs() * ease(if down { p.x - at.x } else { p.y - at.y }, along);
            let u = if down {
                Point::new(0.0, js::sign(along) * k)
            } else {
                Point::new(js::sign(along) * k, 0.0)
            };
            let _ = write!(d, "C{} {} {}", at + u, p - u, p);
            at = p;
            continue;
        }
        if !turns(&pts, i, last) {
            // A point part of the way along one straight run adds nothing to
            // the path.
            if i < last && aligned(&pts, i) && aligned(&pts, i + 1) {
                let (prev, next) = (pts[i - 1].at, pts[i + 1].at);
                if (p.x - prev.x) * (next.x - p.x) + (p.y - prev.y) * (next.y - p.y) > 0.0 {
                    continue;
                }
            }
            line_to(&mut d, &mut at, p);
            continue;
        }
        // How much of the leg before the turn and of the leg after it the
        // turn takes: all of one that ends at a node, short of the straight
        // run into a marker, and half of one it shares.
        let mut before = if turns(&pts, i - 1, last) {
            length(&pts, i) / 2.0
        } else {
            length(&pts, i)
        };
        let mut after = if turns(&pts, i + 1, last) {
            length(&pts, i + 1) / 2.0
        } else {
            length(&pts, i + 1)
        };
        if stepped {
            let r = CORNER.min(before).min(after);
            before = r;
            after = r;
        }
        let u = unit(&pts, i - 1, i, pts[i].axis);
        let v = unit(&pts, i, i + 1, pts[i + 1].axis);
        line_to(&mut d, &mut at, p - u * before);
        at = p + v * after;
        if stepped {
            let _ = write!(d, "Q{p} {at}");
        } else {
            let _ = write!(
                d,
                "C{} {} {}",
                p - u * before * (1.0 - HANDLE),
                p + v * after * (1.0 - HANDLE),
                at
            );
        }
    }
    EdgePath {
        d,
        start,
        start_dir: -start_dir,
        end,
        end_dir,
    }
}

/// An edge label on a patch of the background, so the line does not run
/// through the text.
pub fn edge_label_svg<'a>(
    id: &'a str,
    label: &'a Label,
    at: Point,
    attrs: &'a str,
) -> impl fmt::Display + 'a {
    fragment(move |f| {
        let p = crate::PREFIX;
        let w = label.width + 8.0;
        let h = label.height;
        write!(
            f,
            "<g class=\"{p}-edge-label\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" fill=\"var(--_bg)\"/>{}</g>",
            EscText(id),
            num(at.x - w / 2.0),
            num(at.y - h / 2.0),
            num(w),
            num(h),
            label.svg(at, attrs)
        )
    })
}

/// A self-loop beside a node drawn at `drawn`: out of the side that lies
/// across the flow and back in. `k` counts the loops already on the node, so
/// each reaches a little further. Also places the edge's label beyond the
/// loop.
#[allow(clippy::too_many_arguments)]
pub fn loop_path(
    c: &CNode,
    drawn: Size,
    k: f64,
    sideways: bool,
    base: f64,
    trim: f64,
    e: &mut CEdge,
) -> EdgePath {
    let Point { x: cx, y: cy } = c.center;
    let Size { w, h } = drawn;
    let half = (if sideways { w } else { h }) / 2.0;
    let spread = (half - 4.0).min(8.0 + k * 6.0);
    let reach = base + k * 8.0;
    let len = js::or_one(js::hypot(reach, spread));
    // Local frame: `out` points away from the node, `along` runs along its
    // side.
    let at = |out: f64, along: f64| {
        if sideways {
            Point::new(cx + along, cy + h / 2.0 + out)
        } else {
            Point::new(cx + w / 2.0 + out, cy + along)
        }
    };
    let tx = (reach / len) * trim;
    let ty = (spread / len) * trim;
    e.label_at = if sideways {
        Point::new(cx, cy + h / 2.0 + reach + 4.0 + e.label.h / 2.0)
    } else {
        Point::new(cx + w / 2.0 + reach + 4.0 + e.label.w / 2.0, cy)
    };
    EdgePath {
        d: format!(
            "M{}C{} {} {}",
            at(0.0, -spread),
            at(reach, -spread * 2.0),
            at(reach, spread * 2.0),
            at(tx, spread + ty)
        ),
        start: if sideways {
            Point::new(cx - spread, cy + h / 2.0)
        } else {
            Point::new(cx + w / 2.0, cy - spread)
        },
        start_dir: if sideways {
            Point::new(0.0, -1.0)
        } else {
            Point::new(-1.0, 0.0)
        },
        end: if sideways {
            Point::new(cx + spread, cy + h / 2.0)
        } else {
            Point::new(cx + w / 2.0, cy + spread)
        },
        end_dir: if sideways {
            Point::new(-spread / len, -reach / len)
        } else {
            Point::new(-reach / len, -spread / len)
        },
    }
}
