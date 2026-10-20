//! Pele's `diagrams/architecture/render.ts`.

use crate::config::clamped;
use crate::geom::Point;
use crate::svg::builder::{EscText, TITLE, fragment, icon_element};
use crate::svg::edges::Marker;
use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};
use std::{
    cmp::Ordering,
    collections::HashMap,
    fmt::{self, Write as _},
};

use super::{db::ArchitectureModel, icons::builtin_icon, layout::place_on_grid};
use crate::geom::{Side, Size};
use crate::{
    Config, Ctx, Error, Rendered,
    diagrams::Render,
    js,
    svg::{builder::num, edges::edge_label_svg, root::svg_document, theme::RADIUS},
    text::{Host, Label, Style},
};

const STUB: f64 = 12.0;
const CORNER: f64 = 6.0;
const LABEL_GAP: f64 = 4.0;
const DOT: f64 = 3.0;

/// Maps the grid lines in use to consecutive indexes, so empty rows and
/// columns take no room.
fn rank(mut values: Vec<i64>) -> HashMap<i64, usize> {
    let mut out = HashMap::new();
    values.sort();
    for value in values {
        let next = out.len();
        out.entry(value).or_insert(next);
    }
    out
}

fn icon_slot<'a>(
    name: &'a str,
    at: Point,
    side: f64,
    ctx: &'a Ctx<'_, impl Host>,
) -> impl fmt::Display + 'a {
    let markup = fragment(move |f| match crate::text::icon(ctx.host, name) {
        Some(markup) => f.write_str(&markup),
        None => write!(f, "{}", builtin_icon(name)),
    });
    icon_element(name, at, side, markup)
}

#[derive(Clone, Copy)]
struct Port {
    at: Point,
    /// Out of the side the port is on.
    normal: Point,
    /// The corners of the box the route has to stay out of when it cannot
    /// leave straight.
    lo: Point,
    hi: Point,
    /// How far short of the port an arrowhead stops: the radius of a
    /// junction's dot.
    gap: f64,
    /// Set where a junction is reached through the border of its group: the
    /// line carries on to the junction.
    wire: bool,
}

/// An orthogonal route from one port to another, each left along its side's
/// normal.
fn route(s: &Port, e: &Port) -> Vec<Point> {
    let (sa, sn) = (s.at, s.normal);
    let (ea, en) = (e.at, e.normal);
    let s1 = sa + sn * STUB;
    let e1 = ea + en * STUB;
    if sn.x * en.x + sn.y * en.y == 0.0 {
        let c = if sn.x != 0.0 {
            Point::new(ea.x, sa.y)
        } else {
            Point::new(sa.x, ea.y)
        };
        if (c.x - sa.x) * sn.x + (c.y - sa.y) * sn.y > 0.0
            && (c.x - ea.x) * en.x + (c.y - ea.y) * en.y > 0.0
        {
            return vec![sa, c, ea];
        }
        return if sn.x != 0.0 {
            vec![sa, s1, Point::new(s1.x, e1.y), e1, ea]
        } else {
            vec![sa, s1, Point::new(e1.x, s1.y), e1, ea]
        };
    }
    let sideways = sn.x != 0.0;
    let offset = if sideways { ea.y - sa.y } else { ea.x - sa.x };
    if sn.x == -en.x && sn.y == -en.y {
        let ahead = (ea.x - sa.x) * sn.x + (ea.y - sa.y) * sn.y;
        if ahead > 0.0 {
            if offset.abs() < 0.5 {
                return vec![sa, ea];
            }
            // Turn midway between the two boxes, which is clear of both
            // titles, when there is such a place.
            if sideways {
                let lo = if sn.x > 0.0 { s.hi.x } else { e.hi.x };
                let hi = if sn.x > 0.0 { e.lo.x } else { s.lo.x };
                let mx = if lo < hi {
                    (lo + hi) / 2.0
                } else {
                    (sa.x + ea.x) / 2.0
                };
                return vec![sa, Point::new(mx, sa.y), Point::new(mx, ea.y), ea];
            }
            let lo = if sn.y > 0.0 { s.hi.y } else { e.hi.y };
            let hi = if sn.y > 0.0 { e.lo.y } else { s.lo.y };
            let my = if lo < hi {
                (lo + hi) / 2.0
            } else {
                (sa.y + ea.y) / 2.0
            };
            return vec![sa, Point::new(sa.x, my), Point::new(ea.x, my), ea];
        }
    } else if offset.abs() >= 1.0 {
        if sideways {
            let x = if sn.x > 0.0 {
                sa.x.max(ea.x) + STUB
            } else {
                sa.x.min(ea.x) - STUB
            };
            return vec![sa, Point::new(x, sa.y), Point::new(x, ea.y), ea];
        }
        let y = if sn.y > 0.0 {
            sa.y.max(ea.y) + STUB
        } else {
            sa.y.min(ea.y) - STUB
        };
        return vec![sa, Point::new(sa.x, y), Point::new(ea.x, y), ea];
    }
    // The straight way is blocked by one of the two nodes: go between them,
    // or around both.
    if sideways {
        let lane = if s.hi.y + STUB <= e.lo.y {
            (s.hi.y + e.lo.y) / 2.0
        } else if e.hi.y + STUB <= s.lo.y {
            (e.hi.y + s.lo.y) / 2.0
        } else {
            s.hi.y.max(e.hi.y) + STUB
        };
        return vec![
            sa,
            Point::new(s1.x, sa.y),
            Point::new(s1.x, lane),
            Point::new(e1.x, lane),
            Point::new(e1.x, ea.y),
            ea,
        ];
    }
    let lane = if s.hi.x + STUB <= e.lo.x {
        (s.hi.x + e.lo.x) / 2.0
    } else if e.hi.x + STUB <= s.lo.x {
        (e.hi.x + s.lo.x) / 2.0
    } else {
        s.hi.x.max(e.hi.x) + STUB
    };
    vec![
        sa,
        Point::new(sa.x, s1.y),
        Point::new(lane, s1.y),
        Point::new(lane, e1.y),
        Point::new(ea.x, e1.y),
        ea,
    ]
}

/// Drops repeated points and joins runs that continue in a straight line.
fn simplify(points: &[Point]) -> Vec<Point> {
    let mut out = vec![points[0]];
    for &q in &points[1..] {
        let n = out.len();
        let last = out[n - 1];
        let same_x = (q.x - last.x).abs() < 0.01;
        let same_y = (q.y - last.y).abs() < 0.01;
        if same_x && same_y {
            continue;
        }
        if n >= 2
            && ((same_x && (out[n - 2].x - last.x).abs() < 0.01)
                || (same_y && (out[n - 2].y - last.y).abs() < 0.01))
        {
            out[n - 1] = q;
        } else {
            out.push(q);
        }
    }
    out
}

fn path_data(d: &mut String, points: &[Point], start_trim: f64, end_trim: f64) {
    let mut p = points.to_vec();
    let n = p.len();
    let mut trim = |at: usize, toward: usize, amount: f64| {
        let len = js::hypot(p[toward].x - p[at].x, p[toward].y - p[at].y);
        let t = amount.min(0f64.max(len - 1.0));
        if len > 0.0 && t > 0.0 {
            let step = (p[toward] - p[at]) / len * t;
            p[at] += step;
        }
    };
    trim(0, 1, start_trim);
    trim(n - 1, n - 2, end_trim);
    let _ = write!(d, "M{}", p[0]);
    for i in 1..n - 1 {
        let (into, out) = (p[i] - p[i - 1], p[i + 1] - p[i]);
        let before = js::hypot(into.x, into.y);
        let after = js::hypot(out.x, out.y);
        let r = CORNER.min(before / 2.0).min(after / 2.0);
        if r < 0.5 {
            let _ = write!(d, "L{}", p[i]);
            continue;
        }
        let _ = write!(
            d,
            "L{}Q{} {}",
            p[i] - into / before * r,
            p[i],
            p[i] + out / after * r
        );
    }
    let _ = write!(d, "L{}", p[n - 1]);
}

/// A stretch of an edge that runs up or down.
struct Run {
    x: f64,
    top: f64,
    bottom: f64,
}

/// Mermaid's `architecture` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ArchitectureConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub font_size: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub icon_size: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub padding: Option<f64>,
}

impl Render for ArchitectureModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(ctx.tighten(|tight| self.draw(&config.architecture, ctx, tight)))
    }
}

impl ArchitectureModel {
    fn draw(&self, config: &ArchitectureConfig, ctx: &Ctx<'_, impl Host>, tight: bool) -> Rendered {
        let p = crate::PREFIX;
        let icon_size = clamped(config.icon_size, 8.0, 512.0).unwrap_or(48.0);
        let size = clamped(config.font_size, 4.0, 128.0).unwrap_or(ctx.options.font_size);
        let group_pad = clamped(config.padding, 0.0, 256.0)
            .unwrap_or(20.0)
            .min(if tight { 12.0 } else { f64::INFINITY });
        let small = js::round(size * 0.875);
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);
        let half = icon_size / 2.0;

        let nodes: Vec<_> = self.nodes.values().collect();
        let groups: Vec<_> = self.groups.values().collect();
        let n = nodes.len();
        let g_count = groups.len();
        let grid = place_on_grid(&nodes, &groups, &self.edges, &self.layout_hints);
        let index: HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (node.id.as_str(), i))
            .collect();

        // A service is its icon square with the title beneath; a junction takes
        // the same room.
        let mut labels = Vec::with_capacity(n);
        let mut boxes = Vec::with_capacity(n);
        let wrap = if tight {
            icon_size.max(64.0)
        } else {
            (icon_size * 2.25).max(96.0)
        };
        for node in &nodes {
            let title = if node.is_junction() {
                None
            } else {
                node.title.as_deref()
            };
            let label = Label::layout(title, true, measure, size, wrap, 0);
            boxes.push(Size::new(
                icon_size.max(2.0 * (label.width / 2.0).ceil()),
                icon_size
                    + if label.height > 0.0 {
                        LABEL_GAP + label.height
                    } else {
                        0.0
                    },
            ));
            labels.push(label);
        }

        let mut col_values = Vec::new();
        let mut row_values = Vec::new();
        for node in &grid.nodes {
            col_values.push(node.col);
            row_values.push(node.row);
        }
        for group in grid.groups.iter().filter(|group| group.empty) {
            col_values.push(group.col);
            row_values.push(group.row);
        }
        let col_of = rank(col_values);
        let row_of = rank(row_values);
        let col_at = |value: i64| col_of.get(&value).copied().unwrap_or(0);
        let row_at = |value: i64| row_of.get(&value).copied().unwrap_or(0);
        let c_count = col_of.len();
        let r_count = row_of.len();
        let mut col_w = vec![0.0; c_count];
        let mut row_h = vec![0.0; r_count];
        let gap = if tight {
            24f64.max(2.0 * STUB)
        } else {
            40f64.max(js::round(icon_size * 0.75))
        };
        let mut gap_x = vec![gap; c_count];
        let mut gap_y = vec![gap; r_count];
        let mut node_col = vec![0usize; n];
        let mut node_row = vec![0usize; n];
        for (i, node) in grid.nodes.iter().enumerate() {
            let c = col_at(node.col);
            let r = row_at(node.row);
            node_col[i] = c;
            node_row[i] = r;
            col_w[c] = f64::max(col_w[c], boxes[i].w);
            row_h[r] = f64::max(row_h[r], boxes[i].h);
        }
        for group in grid.groups.iter().filter(|group| group.empty) {
            let c = col_at(group.col);
            let r = row_at(group.row);
            col_w[c] = f64::max(col_w[c], icon_size);
            row_h[r] = f64::max(row_h[r], half);
        }

        // An edge label needs the gap its edge crosses to be wide enough to hold
        // it.
        let edge_count = self.edges.len();
        let mut edge_labels = Vec::with_capacity(edge_count);
        let mut ends: Vec<Option<(usize, usize)>> = Vec::with_capacity(edge_count);
        for edge in &self.edges {
            let a = index.get(edge.lhs_id.as_str()).copied();
            let b = index.get(edge.rhs_id.as_str()).copied();
            let label = Label::layout(edge.title.as_deref(), true, measure, small, 160.0, 0);
            let pair = a.zip(b);
            ends.push(pair);
            let width = label.width;
            let height = label.height;
            edge_labels.push(label);
            let Some((a, b)) = pair else { continue };
            if width == 0.0 {
                continue;
            }
            if node_col[a] != node_col[b] {
                let c = node_col[a].min(node_col[b]);
                gap_x[c] = f64::max(gap_x[c], width.ceil() + 28.0);
            } else {
                col_w[node_col[a]] = f64::max(col_w[node_col[a]], 2.0 * (width / 2.0).ceil() + 8.0);
                if node_row[a] != node_row[b] {
                    let r = node_row[a].min(node_row[b]);
                    gap_y[r] = f64::max(gap_y[r], height.ceil() + 24.0);
                }
            }
        }

        // Room around each group, kept on the rows and columns at its edges. A
        // group that shares an edge with a group inside it stands that much
        // further out. Children come after their parents, so walking backwards
        // settles every group before the one that holds it.
        let mut group_labels = Vec::with_capacity(g_count);
        let mut c0 = vec![0usize; g_count];
        let mut c1 = vec![0usize; g_count];
        let mut r0 = vec![0usize; g_count];
        let mut r1 = vec![0usize; g_count];
        let mut ins_l = vec![0.0; g_count];
        let mut ins_r = vec![0.0; g_count];
        let mut ins_t = vec![0.0; g_count];
        let mut ins_b = vec![0.0; g_count];
        let mut heads = vec![Size::default(); g_count];
        let mut col_l = vec![0.0; c_count];
        let mut col_r = vec![0.0; c_count];
        let mut row_t = vec![0.0; r_count];
        let mut row_b = vec![0.0; r_count];
        let badge = js::round(size * 1.25);
        let group_wrap = if tight {
            80f64.max(ctx.title_room() / 2.0)
        } else {
            320.0
        };
        for g in 0..g_count {
            let cells = &grid.groups[g];
            c0[g] = col_at(cells.col);
            c1[g] = col_at(cells.col + cells.cols - 1);
            r0[g] = row_at(cells.row);
            r1[g] = row_at(cells.row + cells.rows - 1);
            let label = Label::layout(
                groups[g].title.as_deref(),
                true,
                measure,
                size,
                group_wrap,
                0,
            );
            let icon = if has_icon(groups[g].icon.as_deref()) {
                badge
            } else {
                0.0
            };
            heads[g] = Size::new(
                label.width
                    + icon
                    + if label.width > 0.0 && icon > 0.0 {
                        6.0
                    } else {
                        0.0
                    },
                label.height.max(icon),
            );
            group_labels.push(label);
        }
        for g in (0..g_count).rev() {
            let left = group_pad + ins_l[g];
            let mut right = group_pad + ins_r[g];
            let top = if heads[g].h > 0.0 {
                group_pad.max(heads[g].h + 12.0)
            } else {
                group_pad
            } + ins_t[g];
            let bottom = group_pad + ins_b[g];
            if heads[g].w > 0.0 {
                let mut inner = left + right;
                for c in c0[g]..=c1[g] {
                    inner += col_w[c]
                        + if c < c1[g] {
                            col_r[c] + gap_x[c] + col_l[c + 1]
                        } else {
                            0.0
                        };
                }
                if heads[g].w + 20.0 > inner {
                    right += (heads[g].w + 20.0 - inner).ceil();
                }
            }
            ins_l[g] = left;
            ins_r[g] = right;
            ins_t[g] = top;
            ins_b[g] = bottom;
            col_l[c0[g]] = f64::max(col_l[c0[g]], left);
            col_r[c1[g]] = f64::max(col_r[c1[g]], right);
            row_t[r0[g]] = f64::max(row_t[r0[g]], top);
            row_b[r1[g]] = f64::max(row_b[r1[g]], bottom);
            let Some(p) = grid.groups[g].parent else {
                continue;
            };
            if c0[p] == c0[g] {
                ins_l[p] = f64::max(ins_l[p], left);
            }
            if c1[p] == c1[g] {
                ins_r[p] = f64::max(ins_r[p], right);
            }
            if r0[p] == r0[g] {
                ins_t[p] = f64::max(ins_t[p], top);
            }
            if r1[p] == r1[g] {
                ins_b[p] = f64::max(ins_b[p], bottom);
            }
        }

        let mut xs = vec![0.0; c_count];
        let mut ys = vec![0.0; r_count];
        let mut inner_w = 0.0;
        for c in 0..c_count {
            xs[c] = inner_w + col_l[c];
            inner_w = xs[c] + col_w[c] + col_r[c] + if c + 1 < c_count { gap_x[c] } else { 0.0 };
        }
        let mut inner_h = 0.0;
        for r in 0..r_count {
            ys[r] = inner_h + row_t[r];
            inner_h = ys[r] + row_h[r] + row_b[r] + if r + 1 < r_count { gap_y[r] } else { 0.0 };
        }
        // What is drawn, which an edge that has to go around a node can push past
        // the grid.
        let mut min_x: f64 = 0.0;
        let mut min_y: f64 = 0.0;
        let mut max_x: f64 = inner_w;
        let mut max_y: f64 = inner_h;

        let left = |g: usize| xs[c0[g]] - ins_l[g];
        let right = |g: usize| xs[c1[g]] + col_w[c1[g]] + ins_r[g];
        let top = |g: usize| ys[r0[g]] - ins_t[g];
        let bottom = |g: usize| ys[r1[g]] + row_h[r1[g]] + ins_b[g];
        let center_x = |i: usize| xs[node_col[i]] + col_w[node_col[i]] / 2.0;

        let port = |i: usize, side: Side, via_group: bool| -> Port {
            let normal = side.outward();
            let cx = center_x(i);
            let y = ys[node_row[i]];
            let mut p = Port {
                at: Point::new(cx, y + half),
                normal,
                lo: Point::new(cx - boxes[i].w / 2.0, y),
                hi: Point::new(cx + boxes[i].w / 2.0, y + boxes[i].h),
                gap: 0.0,
                wire: false,
            };
            if nodes[i].is_junction() {
                p.lo = p.at;
                p.hi = p.at;
                p.gap = DOT;
            } else if normal.x != 0.0 {
                p.at.x = cx + normal.x * half;
            } else {
                p.at.y = if normal.y < 0.0 {
                    y
                } else {
                    y + boxes[i].h + if labels[i].height > 0.0 { 2.0 } else { 0.0 }
                };
            }
            let g = if via_group {
                grid.nodes[i].parent
            } else {
                None
            };
            if let Some(g) = g {
                p.lo = Point::new(left(g), top(g));
                p.hi = Point::new(right(g), bottom(g));
                p.gap = 0.0;
                p.wire = nodes[i].is_junction();
                if normal.x != 0.0 {
                    p.at.x = if normal.x < 0.0 { p.lo.x } else { p.hi.x };
                } else {
                    p.at.y = if normal.y < 0.0 { p.lo.y } else { p.hi.y };
                }
            }
            p
        };

        let mut edges_out = String::new();
        let mut labels_out = String::new();
        let mut id = String::new();
        // Vertical runs of the edges, to keep group titles out from under them.
        let mut runs: Vec<Run> = Vec::new();
        let trim_length = Marker::Arrow.trim();
        for (k, edge) in self.edges.iter().enumerate() {
            let Some((a, b)) = ends[k] else { continue };
            let mut s = port(a, edge.lhs_dir, edge.lhs_group);
            let mut e = port(b, edge.rhs_dir, edge.rhs_group);
            if s.at == e.at && s.normal.x == e.normal.x {
                // An edge from a side back to the same side: part the two ends so
                // it shows as a loop.
                let part = Point::new(8.0 * s.normal.y.abs(), 8.0 * s.normal.x.abs());
                s.at = s.at - part;
                e.at += part;
            }
            let mut routed = route(&s, &e);
            if s.wire {
                routed.insert(0, Point::new(center_x(a), ys[node_row[a]] + half));
            }
            if e.wire {
                routed.push(Point::new(center_x(b), ys[node_row[b]] + half));
            }
            let points = simplify(&routed);
            if points.len() < 2 {
                continue;
            }
            id.clear();
            let _ = write!(id, "{}-{}", edge.lhs_id, edge.rhs_id);
            let _ = write!(
                edges_out,
                "<g class=\"{p}-edge\" data-id=\"{}\"><path d=\"",
                EscText(&id)
            );
            path_data(
                &mut edges_out,
                &points,
                if edge.lhs_into && !s.wire {
                    trim_length + s.gap
                } else {
                    0.0
                },
                if edge.rhs_into && !e.wire {
                    trim_length + e.gap
                } else {
                    0.0
                },
            );
            edges_out.push_str("\"/>");
            if edge.lhs_into {
                let _ = write!(
                    edges_out,
                    "{}",
                    Marker::Arrow.draw(s.at + s.normal * s.gap, -s.normal, "var(--_l)")
                );
            }
            if edge.rhs_into {
                let _ = write!(
                    edges_out,
                    "{}",
                    Marker::Arrow.draw(e.at + e.normal * e.gap, -e.normal, "var(--_l)")
                );
            }
            edges_out += "</g>";
            let mut best = 0;
            let mut best_length = -1.0;
            let mut flat = 0;
            let mut flat_length = -1.0;
            let label = &edge_labels[k];
            for q in &points {
                min_x = min_x.min(q.x - 4.0);
                max_x = max_x.max(q.x + 4.0);
                min_y = min_y.min(q.y - 4.0);
                max_y = max_y.max(q.y + 4.0);
            }
            for (i, pair) in points.windows(2).enumerate() {
                let (a, b) = (pair[0], pair[1]);
                let horizontal = (a.y - b.y).abs() < 0.01;
                let length = (a.x - b.x).abs() + (a.y - b.y).abs();
                if !horizontal {
                    runs.push(Run {
                        x: a.x,
                        top: a.y.min(b.y),
                        bottom: a.y.max(b.y),
                    });
                }
                if length > best_length {
                    best = i;
                    best_length = length;
                }
                if horizontal && length > flat_length {
                    flat = i;
                    flat_length = length;
                }
            }
            if label.width > 0.0 {
                let at = if flat_length >= label.width + 8.0 {
                    flat
                } else {
                    best
                };
                let (a, b) = (points[at], points[at + 1]);
                let Point { mut x, mut y } = a.midpoint(b);
                let w = label.width + 8.0;
                let upright = (a.x - b.x).abs() < 0.01;
                // On a short run next to a node the label would land on the node,
                // so it moves off to the side.
                for end in [&s, &e] {
                    if x + w / 2.0 <= end.lo.x
                        || x - w / 2.0 >= end.hi.x
                        || y + label.height / 2.0 <= end.lo.y
                        || y - label.height / 2.0 >= end.hi.y
                    {
                        continue;
                    }
                    if upright {
                        x = if x >= (end.lo.x + end.hi.x) / 2.0 {
                            end.hi.x + w / 2.0 + 4.0
                        } else {
                            end.lo.x - w / 2.0 - 4.0
                        };
                    } else {
                        y = if y >= (end.lo.y + end.hi.y) / 2.0 {
                            end.hi.y + label.height / 2.0 + 4.0
                        } else {
                            end.lo.y - label.height / 2.0 - 4.0
                        };
                    }
                }
                min_x = min_x.min(x - w / 2.0);
                max_x = max_x.max(x + w / 2.0);
                min_y = min_y.min(y - label.height / 2.0);
                max_y = max_y.max(y + label.height / 2.0);
                let _ = write!(
                    labels_out,
                    "{}",
                    edge_label_svg(&id, label, Point::new(x, y), "")
                );
            }
        }

        let mut clusters = String::new();
        for (g, group) in groups.iter().enumerate() {
            let x = left(g);
            let y = top(g);
            let w = right(g) - x;
            let label = &group_labels[g];
            let _ = write!(
                clusters,
                "<g class=\"{p}-cluster\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_a)\" fill-opacity=\"0.5\" stroke=\"var(--_b)\"/>",
                EscText(&group.id),
                num(x),
                num(y),
                num(w),
                num(bottom(g) - y),
            );
            let head = heads[g];
            if head.w > 0.0 {
                // The title starts at the left, or after the edges that come down
                // through that corner.
                let band = y + head.h + 12.0;
                let mut crossings: Vec<f64> = Vec::new();
                for run in &runs {
                    if run.top < band && run.bottom > y && run.x > x && run.x < x + w {
                        crossings.push(run.x);
                    }
                }
                crossings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
                crossings.push(x + w - 4.0);
                let mut start = x + 10.0;
                for crossing in crossings {
                    if crossing - 6.0 - start >= head.w {
                        break;
                    }
                    start = start.max(crossing + 6.0);
                }
                if start + head.w > x + w - 4.0 {
                    start = x + 10.0;
                }
                let cy = y + 6.0 + head.h / 2.0;
                let mut text_x = start;
                if let Some(icon) = group.icon.as_deref().filter(|icon| has_icon(Some(icon))) {
                    let _ = write!(
                        clusters,
                        "{}",
                        icon_slot(icon, Point::new(start, cy - badge / 2.0), badge, ctx)
                    );
                    text_x += badge + 6.0;
                }
                let _ = write!(
                    clusters,
                    "{}",
                    label.svg(
                        Point::new(text_x + label.width / 2.0, cy),
                        crate::class!("cluster-label", "fill=\"var(--_m)\"")
                    )
                );
            }
            clusters.push_str("</g>");
        }

        let mut nodes_out = String::new();
        let glyph = icon_size * 0.5;
        let text_size = 6f64.max(js::round(size * 0.75));
        let icon_text_attrs = format!(
            " class=\"{p}-icon-text\" font-size=\"{}\"",
            js::Number(text_size)
        );
        for (i, node) in nodes.iter().enumerate() {
            let at = fragment(|f| {
                write!(
                    f,
                    "data-id=\"{}\" transform=\"translate({},{})\"",
                    EscText(&node.id),
                    num(center_x(i)),
                    num(ys[node_row[i]])
                )
            });
            if node.is_junction() {
                let _ = write!(
                    nodes_out,
                    "<g class=\"{p}-node {p}-junction\" {at}><circle cy=\"{}\" r=\"{}\" fill=\"var(--_l)\"/></g>",
                    num(half),
                    js::Number(DOT)
                );
                continue;
            }
            let _ = write!(
                nodes_out,
                "<g class=\"{p}-node {p}-service\" {at}><rect x=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"/>",
                num(-half),
                num(icon_size),
                num(icon_size),
            );
            if let Some(icon) = node.icon.as_deref().filter(|icon| has_icon(Some(icon))) {
                let _ = write!(
                    nodes_out,
                    "{}",
                    icon_slot(
                        icon,
                        Point::new(-glyph / 2.0, (icon_size - glyph) / 2.0),
                        glyph,
                        ctx
                    )
                );
            } else if let Some(text) = node.icon_text.as_deref().filter(|text| !text.is_empty()) {
                let text = Label::layout(Some(text), false, measure, text_size, icon_size - 8.0, 0);
                let line_height = if text.line_height > 0.0 {
                    text.line_height
                } else {
                    1.0
                };
                let lines = 1f64.max(((icon_size - 4.0) / line_height).floor());
                let _ = write!(
                    nodes_out,
                    "{}",
                    text.ellipsized(icon_size - 8.0, lines, measure)
                        .svg(Point::new(0.0, half), &icon_text_attrs)
                );
            }
            let _ = write!(
                nodes_out,
                "{}</g>",
                labels[i].svg(
                    Point::new(0.0, icon_size + LABEL_GAP + labels[i].height / 2.0),
                    crate::class!("label")
                ),
            );
        }

        let title = Label::layout(
            self.titles.title.as_deref(),
            false,
            measure,
            size,
            if tight { ctx.title_room() } else { 4000.0 },
            Style::BOLD,
        );
        let title_height = title.spaced_height(12.0);
        let width = ((max_x - min_x).max(title.width) + 2.0 * pad).ceil();
        let height = (max_y - min_y + title_height + 2.0 * pad).ceil();
        let ox = js::round((width - (max_x - min_x)) / 2.0 - min_x);
        let oy = js::round(pad + title_height - min_y);
        let mut content = String::with_capacity(
            clusters.len() + edges_out.len() + labels_out.len() + nodes_out.len() + 1024,
        );
        let _ = write!(
            content,
            "{}",
            title.svg(Point::new(width / 2.0, pad + title.height / 2.0), TITLE)
        );
        let any = !(clusters.is_empty()
            && edges_out.is_empty()
            && labels_out.is_empty()
            && nodes_out.is_empty());
        if any {
            let _ = write!(
                content,
                "<g transform=\"translate({},{})\">",
                js::Number(ox),
                js::Number(oy)
            );
        }
        let body = &mut content;
        if !clusters.is_empty() {
            let _ = write!(body, "<g class=\"{p}-clusters\">{clusters}</g>");
        }
        if !edges_out.is_empty() {
            let _ = write!(
                body,
                "<g class=\"{p}-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{edges_out}</g>"
            );
        }
        if !labels_out.is_empty() {
            let _ = write!(
                body,
                "<g class=\"{p}-edge-labels\" font-size=\"{}\">{labels_out}</g>",
                js::Number(small)
            );
        }
        if !nodes_out.is_empty() {
            let _ = write!(body, "<g class=\"{p}-nodes\">{nodes_out}</g>");
        }
        if any {
            content.push_str("</g>");
        }
        let svg = svg_document(
            "architecture",
            width,
            height,
            size,
            ctx,
            self.titles.accessible(),
            &content,
        );
        Rendered {
            svg,
            width,
            height,
            links: Vec::new(),
        }
    }
}

/// JavaScript's truthiness of an optional icon name.
fn has_icon(icon: Option<&str>) -> bool {
    icon.is_some_and(|icon| !icon.is_empty())
}
