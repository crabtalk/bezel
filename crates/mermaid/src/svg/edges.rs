//! Edges: the path of a route, its markers, its label, and loops. Pele's
//! `svg/edges.ts`. Marker types are Mermaid's: `arrow_point`,
//! `arrow_circle`, `arrow_cross`; anything else draws nothing.

use std::{
    collections::HashMap,
    fmt::{self, Write as _},
};

use super::builder::{EscText, LabelSvg, Point, fragment, num};
use crate::{
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EdgePath {
    pub d: String,
    pub sx: f64,
    pub sy: f64,
    pub sdx: f64,
    pub sdy: f64,
    pub ex: f64,
    pub ey: f64,
    pub edx: f64,
    pub edy: f64,
}

/// How far from each side of each node the edges that meet it start to
/// turn: as far as the largest marker there needs, for every edge on the
/// side, so none turns sooner than its neighbour and crosses it.
#[derive(Default)]
pub struct SideReach {
    reach: HashMap<i64, f64>,
}

impl SideReach {
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes a node, the side of it, and the marker of an end there.
    pub fn add(&mut self, node: i64, side: i32, kind: &str) {
        let trim = marker_trim(kind);
        let key = node * 4 + i64::from(side);
        if side >= 0 && trim > 0.0 && trim + LEAD > self.reach.get(&key).copied().unwrap_or(0.0) {
            self.reach.insert(key, trim + LEAD);
        }
    }

    pub fn of(&self, node: i64, side: i32) -> f64 {
        if side >= 0 {
            self.reach
                .get(&(node * 4 + i64::from(side)))
                .copied()
                .unwrap_or(0.0)
        } else {
            0.0
        }
    }
}

/// `x || 1`: a zero (or NaN) as one.
fn or_one(x: f64) -> f64 {
    if x == 0.0 || x.is_nan() { 1.0 } else { x }
}

/// The path for a route of x, y, axis triples, trimmed at both ends for
/// markers. Between two points not in line, a route is one S that leaves and
/// arrives along the flow, or a step in a stepped route. Where the route
/// turns a corner, a stepped route rounds it a little and any other but a
/// linear one turns as widely as it can: the turn takes the whole of a leg
/// that ends at a node and half of a leg it shares with the next turn.
/// `start_reach` and `end_reach` say how far from its nodes the edge starts
/// to turn, where other edges on the same side of a node need it to; by
/// default, far enough to clear its own marker.
pub fn route_path(
    route: &[f64],
    curve: Option<&str>,
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
    let linear = curve == Some("linear");
    let stepped = matches!(curve, Some("step" | "stepBefore" | "stepAfter"));
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    let mut axes: Vec<f64> = Vec::new();
    let near =
        |ax: f64, ay: f64, bx: f64, by: f64| (ax - bx).abs() < 0.01 && (ay - by).abs() < 0.01;
    for triple in route.chunks(3) {
        let (x, y, axis) = (triple[0], triple[1], triple[2]);
        // A stepped route turns at right angles where a smooth one would
        // curve.
        if stepped
            && let (Some(&px), Some(&py)) = (xs.last(), ys.last())
            && (x - px).abs() >= 0.01
            && (y - py).abs() >= 0.01
        {
            let my = match curve {
                Some("stepBefore") => py,
                Some("stepAfter") => y,
                _ => (py + y) / 2.0,
            };
            let mx = match curve {
                Some("stepBefore") => px,
                Some("stepAfter") => x,
                _ => (px + x) / 2.0,
            };
            let turns = if axis == 0.0 {
                [px, my, x, my]
            } else {
                [mx, py, mx, y]
            };
            for k in [0, 2] {
                if near(turns[k], turns[k + 1], px, py) || near(turns[k], turns[k + 1], x, y) {
                    continue;
                }
                xs.push(turns[k]);
                ys.push(turns[k + 1]);
                axes.push(axis);
            }
        }
        xs.push(x);
        ys.push(y);
        axes.push(axis);
    }
    let mut last = xs.len() - 1;

    let aligned = |xs: &[f64], ys: &[f64], i: usize| {
        (xs[i] - xs[i - 1]).abs() < 0.01 || (ys[i] - ys[i - 1]).abs() < 0.01
    };
    let straight = |xs: &[f64], ys: &[f64], i: usize| linear || aligned(xs, ys, i);
    // The way the route heads from point i to point j: along the flow where
    // an S joins them.
    let unit = |xs: &[f64], ys: &[f64], i: usize, j: usize, axis: f64| -> (f64, f64) {
        if !straight(xs, ys, i.max(j)) {
            return if axis == 0.0 {
                (0.0, or_one(js::sign(ys[j] - ys[i])))
            } else {
                (or_one(js::sign(xs[j] - xs[i])), 0.0)
            };
        }
        let dx = xs[j] - xs[i];
        let dy = ys[j] - ys[i];
        let len = or_one(js::hypot(dx, dy));
        (dx / len, dy / len)
    };
    let (sdx, sdy) = unit(&xs, &ys, 0, 1, axes[1]);
    let (edx, edy) = unit(&xs, &ys, last - 1, last, axes[last]);
    let sx = xs[0];
    let sy = ys[0];
    let ex = xs[last];
    let ey = ys[last];
    xs[0] += sdx * start_trim;
    ys[0] += sdy * start_trim;
    xs[last] -= edx * end_trim;
    ys[last] -= edy * end_trim;

    let length =
        |xs: &[f64], ys: &[f64], i: usize| (xs[i] - xs[i - 1]).abs() + (ys[i] - ys[i - 1]).abs();
    // Whether the route turns a corner at point i, from running one way to
    // running the other.
    let turns = |xs: &[f64], ys: &[f64], i: usize, last: usize| {
        !linear
            && i > 0
            && i < last
            && aligned(xs, ys, i)
            && aligned(xs, ys, i + 1)
            && length(xs, ys, i) >= 0.01
            && length(xs, ys, i + 1) >= 0.01
            && ((xs[i] - xs[i - 1]).abs() < 0.01) != ((xs[i + 1] - xs[i]).abs() < 0.01)
    };
    // An edge that curves into a marker runs straight for a little way
    // first, where it has the room, so the marker does not sit on the bend.
    let along = |xs: &[f64], ys: &[f64], i: usize, ux: f64, uy: f64| {
        (xs[i] - xs[i - 1]).abs() * ux.abs() + (ys[i] - ys[i - 1]).abs() * uy.abs()
    };
    // Of the way from the end of a run to where it turns, the part that
    // makes up for a larger marker beside this one is kept where it can be,
    // and the rest is at most a quarter of what is left, so ends on one side
    // of a node turn equally far from it.
    let lead = |reach: f64, trim: f64, run: f64| {
        let room = 0f64.max(reach - LEAD - trim).min(run / 4.0);
        room + (reach - trim - room).min((run - room) / 4.0)
    };
    let arrives =
        if end_reach > end_trim && (!straight(&xs, &ys, last) || turns(&xs, &ys, last - 1, last)) {
            lead(end_reach, end_trim, along(&xs, &ys, last, edx, edy))
        } else {
            0.0
        };
    let leaves = if start_reach > start_trim && (!straight(&xs, &ys, 1) || turns(&xs, &ys, 1, last))
    {
        lead(start_reach, start_trim, along(&xs, &ys, 1, sdx, sdy))
    } else {
        0.0
    };
    if arrives >= 1.0 {
        let (x, y, axis) = (
            xs[last] - edx * arrives,
            ys[last] - edy * arrives,
            axes[last],
        );
        xs.insert(last, x);
        ys.insert(last, y);
        axes.insert(last, axis);
        last += 1;
    }
    if leaves >= 1.0 {
        let (x, y, axis) = (xs[0] + sdx * leaves, ys[0] + sdy * leaves, axes[1]);
        xs.insert(1, x);
        ys.insert(1, y);
        axes.insert(1, axis);
        last += 1;
    }

    let mut d = String::new();
    let _ = write!(d, "M{}", Point(xs[0], ys[0]));
    let mut at_x = xs[0];
    let mut at_y = ys[0];
    let line_to = |d: &mut String, at_x: &mut f64, at_y: &mut f64, x: f64, y: f64| {
        if near(x, y, *at_x, *at_y) {
            return;
        }
        let _ = write!(d, "L{},{}", num(x), num(y));
        *at_x = x;
        *at_y = y;
    };
    for i in 1..=last {
        let x = xs[i];
        let y = ys[i];
        if !straight(&xs, &ys, i) {
            let down = axes[i] == 0.0;
            let along = if down { y - at_y } else { x - at_x };
            let k = along.abs() * ease(if down { x - at_x } else { y - at_y }, along);
            let ux = if down { 0.0 } else { js::sign(along) * k };
            let uy = if down { js::sign(along) * k } else { 0.0 };
            let _ = write!(
                d,
                "C{},{} {},{} {},{}",
                num(at_x + ux),
                num(at_y + uy),
                num(x - ux),
                num(y - uy),
                num(x),
                num(y)
            );
            at_x = x;
            at_y = y;
            continue;
        }
        if !turns(&xs, &ys, i, last) {
            // A point part of the way along one straight run adds nothing to
            // the path.
            if i < last
                && aligned(&xs, &ys, i)
                && aligned(&xs, &ys, i + 1)
                && (x - xs[i - 1]) * (xs[i + 1] - x) + (y - ys[i - 1]) * (ys[i + 1] - y) > 0.0
            {
                continue;
            }
            line_to(&mut d, &mut at_x, &mut at_y, x, y);
            continue;
        }
        // How much of the leg before the turn and of the leg after it the
        // turn takes: all of one that ends at a node, short of the straight
        // run into a marker, and half of one it shares.
        let mut before = if turns(&xs, &ys, i - 1, last) {
            length(&xs, &ys, i) / 2.0
        } else {
            length(&xs, &ys, i)
        };
        let mut after = if turns(&xs, &ys, i + 1, last) {
            length(&xs, &ys, i + 1) / 2.0
        } else {
            length(&xs, &ys, i + 1)
        };
        if stepped {
            let r = CORNER.min(before).min(after);
            before = r;
            after = r;
        }
        let (ux, uy) = unit(&xs, &ys, i - 1, i, axes[i]);
        let (vx, vy) = unit(&xs, &ys, i, i + 1, axes[i + 1]);
        line_to(
            &mut d,
            &mut at_x,
            &mut at_y,
            x - ux * before,
            y - uy * before,
        );
        at_x = x + vx * after;
        at_y = y + vy * after;
        if stepped {
            let _ = write!(d, "Q{},{} {},{}", num(x), num(y), num(at_x), num(at_y));
        } else {
            let _ = write!(
                d,
                "C{},{} {},{} {},{}",
                num(x - ux * before * (1.0 - HANDLE)),
                num(y - uy * before * (1.0 - HANDLE)),
                num(x + vx * after * (1.0 - HANDLE)),
                num(y + vy * after * (1.0 - HANDLE)),
                num(at_x),
                num(at_y)
            );
        }
    }
    EdgePath {
        d,
        sx,
        sy,
        sdx: -sdx,
        sdy: -sdy,
        ex,
        ey,
        edx,
        edy,
    }
}

pub fn marker_trim(kind: &str) -> f64 {
    match kind {
        "arrow_point" => ARROW - 1.0,
        "arrow_circle" => ARROW,
        "arrow_cross" => 4.0,
        _ => 0.0,
    }
}

/// A marker whose tip is at (`x`, `y`), pointing along (`dx`, `dy`).
pub fn marker<'a>(
    kind: &'a str,
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    color: &'a str,
) -> impl fmt::Display + 'a {
    fragment(move |f| {
        let p = crate::PREFIX;
        let nx = -dy;
        let ny = dx;
        match kind {
            "arrow_point" => {
                let bx = x - dx * ARROW;
                let by = y - dy * ARROW;
                let half = ARROW * 0.42;
                write!(
                    f,
                    "<path class=\"{p}-marker\" d=\"M{}L{}L{}Z\" fill=\"{color}\" stroke=\"none\"/>",
                    Point(x, y),
                    Point(bx + nx * half, by + ny * half),
                    Point(bx - nx * half, by - ny * half)
                )
            }
            "arrow_circle" => write!(
                f,
                "<circle class=\"{p}-marker\" cx=\"{}\" cy=\"{}\" r=\"3.5\" fill=\"{color}\" stroke=\"none\"/>",
                num(x - dx * 4.0),
                num(y - dy * 4.0)
            ),
            "arrow_cross" => {
                let cx = x - dx * 4.0;
                let cy = y - dy * 4.0;
                let a = 3.5;
                let ux = (dx + nx) * a;
                let uy = (dy + ny) * a;
                let vx = (dx - nx) * a;
                let vy = (dy - ny) * a;
                write!(
                    f,
                    "<path class=\"{p}-marker\" d=\"M{}L{}M{}L{}\"/>",
                    Point(cx - ux, cy - uy),
                    Point(cx + ux, cy + uy),
                    Point(cx - vx, cy - vy),
                    Point(cx + vx, cy + vy)
                )
            }
            _ => Ok(()),
        }
    })
}

/// An edge label on a patch of the background, so the line does not run
/// through the text.
pub fn edge_label_svg<'a>(
    id: &'a str,
    label: &'a Label,
    x: f64,
    y: f64,
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
            num(x - w / 2.0),
            num(y - h / 2.0),
            num(w),
            num(h),
            LabelSvg(label, x, y, attrs)
        )
    })
}

/// A self-loop beside a node of drawn size `w` by `h`: out of the side that
/// lies across the flow and back in. `k` counts the loops already on the
/// node, so each reaches a little further. Also places the edge's label
/// beyond the loop.
#[allow(clippy::too_many_arguments)]
pub fn loop_path(
    c: &CNode,
    w: f64,
    h: f64,
    k: f64,
    sideways: bool,
    base: f64,
    trim: f64,
    e: &mut CEdge,
) -> EdgePath {
    let (cx, cy) = (c.x, c.y);
    let half = (if sideways { w } else { h }) / 2.0;
    let spread = (half - 4.0).min(8.0 + k * 6.0);
    let reach = base + k * 8.0;
    let len = or_one(js::hypot(reach, spread));
    // Local frame: `out` points away from the node, `along` runs along its
    // side.
    let at = |out: f64, along: f64| {
        if sideways {
            Point(cx + along, cy + h / 2.0 + out)
        } else {
            Point(cx + w / 2.0 + out, cy + along)
        }
    };
    let tx = (reach / len) * trim;
    let ty = (spread / len) * trim;
    if sideways {
        e.label_x = cx;
        e.label_y = cy + h / 2.0 + reach + 4.0 + e.label_h / 2.0;
    } else {
        e.label_x = cx + w / 2.0 + reach + 4.0 + e.label_w / 2.0;
        e.label_y = cy;
    }
    EdgePath {
        d: format!(
            "M{}C{} {} {}",
            at(0.0, -spread),
            at(reach, -spread * 2.0),
            at(reach, spread * 2.0),
            at(tx, spread + ty)
        ),
        sx: if sideways { cx - spread } else { cx + w / 2.0 },
        sy: if sideways { cy + h / 2.0 } else { cy - spread },
        sdx: if sideways { 0.0 } else { -1.0 },
        sdy: if sideways { -1.0 } else { 0.0 },
        ex: if sideways { cx + spread } else { cx + w / 2.0 },
        ey: if sideways { cy + h / 2.0 } else { cy + spread },
        edx: if sideways {
            -spread / len
        } else {
            -reach / len
        },
        edy: if sideways {
            -reach / len
        } else {
            -spread / len
        },
    }
}
