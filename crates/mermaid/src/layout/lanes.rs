//! Layout for swimlanes. Every top-level group is a lane: a band that runs
//! the length of the diagram along the flow. Ranks are shared by all lanes, so
//! a step sits level with the steps that happen at the same time in other
//! lanes, and each node stays inside its own lane.

use std::collections::HashSet;

use crate::js::{self, compare};

use super::{
    compound::{CEdge, CNode},
    layered::{Kind, LEdge, LNode, LayeredOptions, orient, rank},
};
use crate::geom::{Axis, Direction, Point, RoutePoint, Size};

const LANE_HEAD: f64 = 34.0;
const LANE_PAD: f64 = 24.0;
const SWEEPS: usize = 8;
const MAX_DUMMIES: i64 = 20000;
const MAX_HOPS: usize = 10000;
const ROUTE_BUDGET: usize = 2000000;
/// Parallel lines closer than this are drawn as one.
const NEAR: f64 = 4.0;
const ROUNDS: usize = 3;
const RELAY: usize = 8;
const TURN: f64 = 20.0;
/// The room a lane keeps beside a handoff along its margin, and how much of
/// it is between the handoff and the lane's items.
const MARGIN: f64 = 20.0;
const LEG: f64 = 14.0;
const CLOSE: f64 = 3.0;

/// What one placing of the lanes comes to.
struct Placed {
    size: Size,
    routed: Vec<bool>,
    margins: Vec<bool>,
    work: usize,
}

/// What every placing of the lanes starts from: the ranked items and the
/// relations between them.
struct Lanes<'a> {
    opt: &'a LayeredOptions,
    sideways: bool,
    flipped: bool,
    lanes: Vec<usize>,
    lane_count: usize,
    lane_slot: Vec<Option<usize>>,
    lane_pads: Vec<f64>,
    end_pad: f64,
    item_of: Vec<Option<usize>>,
    real: Vec<usize>,
    lnodes: Vec<LNode>,
    ledges: Vec<LEdge>,
    edge_of: Vec<usize>,
    item_lane: Vec<i32>,
    step: i32,
}

/// Where the items of one placing are, for routing handoffs. Points in the
/// lane frame have `x` across the lanes and `y` along the flow.
struct Router<'a> {
    c: &'a [f64],
    size: &'a [f64],
    depth: &'a [f64],
    rank: &'a [i32],
    band_start: &'a [f64],
    band_end: &'a [f64],
    real: usize,
    sideways: bool,
    flipped: bool,
    head: f64,
    length: f64,
    breadth_total: f64,
    /// The axis the flow runs along in the drawing.
    axis: Axis,
}

impl Router<'_> {
    fn flip(&self, a: f64) -> f64 {
        if self.flipped {
            self.head + self.length - a
        } else {
            a
        }
    }

    /// The point `c` across the lanes and `a` along the flow, in the drawing.
    fn point(&self, c: f64, a: f64) -> Point {
        if self.sideways {
            Point::new(self.flip(a), c)
        } else {
            Point::new(c, self.flip(a))
        }
    }

    /// [`Router::point`] of a point in the lane frame.
    fn place(&self, p: Point) -> Point {
        self.point(p.x, p.y)
    }

    fn amid(&self, i: usize) -> f64 {
        let r = self.rank[i] as usize;
        (self.band_start[r] + self.band_end[r]) / 2.0
    }

    /// Whether the box from `min` to `max` overlaps the box of `size` centred
    /// on `center`.
    fn overlaps(min: Point, max: Point, center: Point, size: Size) -> bool {
        max.x > center.x - size.w / 2.0
            && min.x < center.x + size.w / 2.0
            && max.y > center.y - size.h / 2.0
            && min.y < center.y + size.h / 2.0
    }

    /// Whether the box from `min` to `max` in the lane frame keeps clear of
    /// every item but `src` and `dst`, and of every label but `ei`'s.
    #[allow(clippy::too_many_arguments)]
    fn clear(
        &self,
        edges: &[CEdge],
        labels: &[Point],
        min: Point,
        max: Point,
        src: Option<usize>,
        dst: Option<usize>,
        ei: usize,
    ) -> bool {
        for i in 0..self.real {
            if Some(i) != src
                && Some(i) != dst
                && Self::overlaps(
                    min,
                    max,
                    Point::new(self.c[i], self.amid(i)),
                    Size::new(self.size[i] + 8.0, self.depth[i] + 8.0),
                )
            {
                return false;
            }
        }
        for (i, e) in edges.iter().enumerate() {
            // A step's own loop keeps its label inside the step's box, and
            // has no place here.
            if i != ei
                && e.label.w > 0.0
                && e.src != e.dst
                && Self::overlaps(
                    min,
                    max,
                    labels[i],
                    if self.sideways {
                        e.label.turned()
                    } else {
                        e.label
                    },
                )
            {
                return false;
            }
        }
        true
    }

    /// How many other relations have a stretch inside the box from `min` to
    /// `max`. A stretch's whole box counts, including its curve.
    fn crossings(edges: &[CEdge], min: Point, max: Point, ei: usize) -> usize {
        let mut count = 0;
        for (i, edge) in edges.iter().enumerate() {
            if i == ei {
                continue;
            }
            for pair in edge.route.windows(2) {
                let (p, q) = (pair[0].at, pair[1].at);
                if p.x.max(q.x) > min.x
                    && p.x.min(q.x) < max.x
                    && p.y.max(q.y) > min.y
                    && p.y.min(q.y) < max.y
                {
                    count += 1;
                    break;
                }
            }
        }
        count
    }

    /// How many other relations start or end within a pixel of `p`.
    fn meets(edges: &[CEdge], p: Point, ei: usize) -> usize {
        let mut count = 0;
        for (i, edge) in edges.iter().enumerate() {
            let (Some(first), Some(last)) = (edge.route.first(), edge.route.last()) else {
                continue;
            };
            if i == ei {
                continue;
            }
            let meets = |q: Point| (q.x - p.x).abs() < 1.0 && (q.y - p.y).abs() < 1.0;
            if meets(first.at) || meets(last.at) {
                count += 1;
            }
        }
        count
    }

    /// Two relations drawn along one line read as a single path. Those that
    /// leave the same step or arrive at the same step may share a trunk; any
    /// other pair must not.
    fn retraces(edges: &[CEdge], from: Point, to: Point, ei: usize) -> bool {
        let upright = (to.x - from.x).abs() < 0.5;
        let (across, along) = if upright {
            (Axis::X, Axis::Y)
        } else {
            (Axis::Y, Axis::X)
        };
        let lo = from.along(along).min(to.along(along));
        let hi = from.along(along).max(to.along(along));
        let line = from.along(across);
        for (i, other) in edges.iter().enumerate() {
            if i == ei || other.src == edges[ei].src || other.dst == edges[ei].dst {
                continue;
            }
            for pair in other.route.windows(2) {
                let (p, q) = (pair[0].at, pair[1].at);
                let u0 = p.along(across);
                let u1 = q.along(across);
                if (u0 - line).abs() >= NEAR || (u1 - line).abs() >= NEAR {
                    continue;
                }
                let v0 = p.along(along);
                let v1 = q.along(along);
                if hi.min(v0.max(v1)) - lo.max(v0.min(v1)) > 1.0 {
                    return true;
                }
            }
        }
        false
    }

    /// Tries the route through `pts`, in the lane frame, for relation `k`,
    /// and takes it when it keeps `margin` clear of what is in its way.
    #[allow(clippy::too_many_arguments)]
    fn attempt(
        &self,
        edges: &mut [CEdge],
        labels: &mut [Point],
        routed: &mut [bool],
        e: &LEdge,
        k: usize,
        ei: usize,
        pts: &[Point],
        margin: f64,
    ) -> bool {
        let (a, b) = (Some(e.tail), Some(e.head));
        for pair in pts.windows(2) {
            let (p, q) = (pair[0], pair[1]);
            if !self.clear(
                edges,
                labels,
                Point::new(p.x.min(q.x) - margin, p.y.min(q.y) - margin),
                Point::new(p.x.max(q.x) + margin, p.y.max(q.y) + margin),
                a,
                b,
                ei,
            ) {
                return false;
            }
            if Self::retraces(edges, self.place(p), self.place(q), ei) {
                return false;
            }
        }
        let mut at = Point::default();
        let mut longest = -1.0;
        let out = edges[ei].label;
        if out.w > 0.0 {
            let room = e.label;
            for pair in pts.windows(2) {
                let (p, q) = (pair[0], pair[1]);
                let mid = p.midpoint(q);
                let across = (q.x - p.x).abs();
                let along = (q.y - p.y).abs();
                let run = across + along;
                if run <= longest
                    || (if across > 0.0 {
                        across < room.w + 8.0
                    } else {
                        along < room.h + 8.0
                    })
                {
                    continue;
                }
                let min = Point::new(mid.x - room.w / 2.0, mid.y - room.h / 2.0);
                let max = Point::new(mid.x + room.w / 2.0, mid.y + room.h / 2.0);
                if min.x < 0.0 || max.x > self.breadth_total || min.y < 0.0 || max.y > self.length {
                    continue;
                }
                // Include the ends here: the label must clear both shapes it
                // connects.
                if !self.clear(edges, labels, min, max, None, None, ei) {
                    continue;
                }
                // A label moved onto a handoff must not cover a return path
                // that keeps its ranked route. A segment's whole box is kept
                // clear, including its curve.
                let l = self.place(mid);
                if Self::crossings(
                    edges,
                    Point::new(l.x - out.w / 2.0, l.y - out.h / 2.0),
                    Point::new(l.x + out.w / 2.0, l.y + out.h / 2.0),
                    ei,
                ) > 0
                {
                    continue;
                }
                at = mid;
                longest = run;
            }
            if longest < 0.0 {
                return false;
            }
        }
        // A point short of each corner and one past it keep the turn close to
        // the corner, so that the line stays under its label and the handoff
        // takes little room.
        let mut way = vec![pts[0]];
        for i in 1..pts.len() {
            let (p, q) = (pts[i - 1], pts[i]);
            let dx = q.x - p.x;
            let dy = q.y - p.y;
            let run = dx.abs() + dy.abs();
            let first = if i > 1 { TURN } else { 0.0 };
            let last = if i + 1 < pts.len() { TURN } else { 0.0 };
            if run > first + last + 1.0 {
                if first != 0.0 {
                    way.push(Point::new(
                        p.x + (dx / run) * first,
                        p.y + (dy / run) * first,
                    ));
                }
                if last != 0.0 {
                    way.push(Point::new(q.x - (dx / run) * last, q.y - (dy / run) * last));
                }
            }
            way.push(q);
        }
        let route = way
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                let crosswise = if i == 0 {
                    way[0].x != way[1].x
                } else {
                    p.x != way[i - 1].x
                };
                RoutePoint::new(
                    self.place(p),
                    if crosswise {
                        self.axis.other()
                    } else {
                        self.axis
                    },
                )
            })
            .collect();
        let out = &mut edges[ei];
        out.route = route;
        if out.label.w > 0.0 {
            out.label_at = self.place(at);
            labels[ei] = at;
        }
        routed[k] = true;
        true
    }
}

/// The runs of items of one rank that share a lane, first and last.
fn runs(row: &[usize], lane: &[i32]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut from = 0;
    while from < row.len() {
        let mut to = from;
        while to + 1 < row.len() && lane[row[to + 1]] == lane[row[from]] {
            to += 1;
        }
        out.push((from, to));
        from = to + 1;
    }
    out
}

fn settle_row(row: &mut [usize], lane: &[i32], key: &[f64], pos: &mut [f64]) {
    row.sort_by(|&a, &b| {
        lane[a]
            .cmp(&lane[b])
            .then(compare(key[a] - key[b]))
            .then(a.cmp(&b))
    });
    for (k, &item) in row.iter().enumerate() {
        pos[item] = k as f64;
    }
}

impl Lanes<'_> {
    /// Lays the ranked items out and routes the relations. A plain relation
    /// is a handoff that takes a simple route of its own, and so keeps no
    /// room through the ranks it passes.
    fn place(
        &self,
        nodes: &mut [CNode],
        edges: &mut [CEdge],
        plain: &[bool],
        margins: &[bool],
    ) -> Placed {
        let opt = self.opt;
        let n = nodes.len();
        let m = self.ledges.len();
        let real = &self.real;
        let lnodes = &self.lnodes;
        let ledges = &self.ledges;
        let lane_count = self.lane_count;
        let lanes = &self.lanes;
        let step = self.step;
        let sideways = self.sideways;
        let pad: Vec<f64> = self
            .lane_pads
            .iter()
            .enumerate()
            .map(|(lane, &room)| {
                if margins[lane] {
                    room.max(MARGIN)
                } else {
                    room
                }
            })
            .collect();
        let mut used = vec![false; lane_count];
        // Items: the real ones, then a dummy for every rank an edge passes
        // through.
        let mut dummies = 0i64;
        let mut span = vec![0i32; m];
        for k in 0..m {
            span[k] = (lnodes[ledges[k].head].rank - lnodes[ledges[k].tail].rank).abs();
            if !plain[k] {
                dummies += span[k] as i64 - 1;
            }
        }
        let direct = dummies > MAX_DUMMIES;
        let total = (real.len() as i64 + if direct { 0 } else { dummies }) as usize;
        let mut rank_ = vec![0i32; total];
        let mut lane_ = vec![0i32; total];
        let mut size = vec![0.0; total];
        let mut depth = vec![0.0; total];
        let mut dummy = vec![false; total];
        let mut max_rank = 0;
        for k in 0..real.len() {
            rank_[k] = lnodes[k].rank;
            lane_[k] = self.item_lane[k];
            size[k] = lnodes[k].size.w;
            depth[k] = lnodes[k].size.h;
            if rank_[k] > max_rank {
                max_rank = rank_[k];
            }
        }
        let max_rank = max_rank as usize;
        let mut first_dummy: Vec<Option<usize>> = vec![None; m];
        let mut label_item: Vec<Option<usize>> = vec![None; m];
        let mut links: Vec<usize> = Vec::new();
        {
            let mut next = real.len();
            for k in 0..m {
                let e = &ledges[k];
                let a = if e.reversed { e.head } else { e.tail };
                let b = if e.reversed { e.tail } else { e.head };
                if direct || span[k] < 2 || plain[k] {
                    links.extend([a, b]);
                    continue;
                }
                let ra = rank_[a];
                let rb = rank_[b];
                let mut label_rank = -1;
                if e.label.w > 0.0 {
                    label_rank = ra + ((rb - ra) >> 1);
                    if step == 2 && (label_rank & 1) == 0 {
                        label_rank -= 1;
                    }
                }
                first_dummy[k] = Some(next);
                let mut prev = a;
                for r in ra + 1..rb {
                    let d = next;
                    next += 1;
                    rank_[d] = r;
                    // The edge crosses from its first lane to its last one
                    // evenly along the way.
                    lane_[d] = lane_[a]
                        + js::round(((lane_[b] - lane_[a]) * (r - ra)) as f64 / (rb - ra) as f64)
                            as i32;
                    dummy[d] = true;
                    if r == label_rank {
                        size[d] = e.label.w;
                        depth[d] = e.label.h;
                        label_item[k] = Some(d);
                    }
                    links.extend([prev, d]);
                    prev = d;
                }
                links.extend([prev, b]);
            }
        }

        let mut up_start = vec![0usize; total + 1];
        let mut down_start = vec![0usize; total + 1];
        for k in (0..links.len()).step_by(2) {
            down_start[links[k] + 1] += 1;
            up_start[links[k + 1] + 1] += 1;
        }
        for i in 0..total {
            up_start[i + 1] += up_start[i];
            down_start[i + 1] += down_start[i];
        }
        let mut up = vec![0usize; links.len() / 2];
        let mut down = vec![0usize; links.len() / 2];
        {
            let mut uf = up_start[..total].to_vec();
            let mut df = down_start[..total].to_vec();
            for k in (0..links.len()).step_by(2) {
                down[df[links[k]]] = links[k + 1];
                df[links[k]] += 1;
                up[uf[links[k + 1]]] = links[k];
                uf[links[k + 1]] += 1;
            }
        }

        // Order within each rank: by lane first, then by where the item's
        // neighbours sit.
        let mut rows: Vec<Vec<usize>> = vec![Vec::new(); max_rank + 1];
        for i in 0..total {
            rows[rank_[i] as usize].push(i);
        }
        let mut pos = vec![0.0; total];
        let mut key = vec![0.0; total];
        for row in rows.iter_mut() {
            for (k, &item) in row.iter().enumerate() {
                key[item] = k as f64;
            }
            settle_row(row, &lane_, &key, &mut pos);
        }
        let mut sweep = 0;
        while sweep < SWEEPS && max_rank > 0 {
            let downward = (sweep & 1) == 0;
            for s in 1..=max_rank {
                let row = &mut rows[if downward { s } else { max_rank - s }];
                let start = if downward { &up_start } else { &down_start };
                let list = if downward { &up } else { &down };
                for &i in row.iter() {
                    let mut sum = 0.0;
                    let count = start[i + 1] - start[i];
                    for k in start[i]..start[i + 1] {
                        sum += pos[list[k]];
                    }
                    key[i] = if count > 0 {
                        sum / count as f64
                    } else {
                        pos[i]
                    };
                }
                settle_row(row, &lane_, &key, &mut pos);
            }
            sweep += 1;
        }

        // The items of one rank that share a lane sit side by side as a run.
        let gap = |a: usize, b: usize| {
            if dummy[a] || dummy[b] {
                opt.edge_sep
            } else {
                opt.node_sep
            }
        };
        let run_size = |row: &[usize], from: usize, to: usize| -> f64 {
            let mut total = size[row[from]];
            for k in from..to {
                total += gap(row[k], row[k + 1]) + size[row[k + 1]];
            }
            total
        };

        // Lane breadth: enough for its fullest rank, and for its title.
        let mut breadth = vec![0.0; lane_count];
        let mut head = if !lanes.is_empty() { LANE_HEAD } else { 0.0 };
        for k in 0..lanes.len() {
            let lane = &nodes[lanes[k]];
            breadth[k] = 0f64.max(lane.min_w - 2.0 * pad[k]);
            if lane.pad_top > head {
                head = lane.pad_top;
            }
        }
        for row in &rows {
            for (from, to) in runs(row, &lane_) {
                let total = run_size(row, from, to);
                let lane = lane_[row[from]] as usize;
                if total > breadth[lane] {
                    breadth[lane] = total;
                }
            }
        }
        let mut lane_start = vec![0.0; lane_count + 1];
        for k in 0..lane_count {
            lane_start[k + 1] = lane_start[k] + breadth[k] + 2.0 * pad[k];
        }

        // Across the flow: each lane's items start centered, then line up
        // with their neighbours in the same lane as far as the lane and the
        // items beside them allow.
        let mut c = vec![0.0; total];
        let mut want = vec![0.0; total];
        for row in &rows {
            for (from, to) in runs(row, &lane_) {
                let lane = lane_[row[from]] as usize;
                let mut at = (lane_start[lane] + pad[lane] + (lane_start[lane + 1] - pad[lane])
                    - run_size(row, from, to))
                    / 2.0;
                for k in from..=to {
                    c[row[k]] = at + size[row[k]] / 2.0;
                    if k < to {
                        at += size[row[k]] + gap(row[k], row[k + 1]);
                    }
                }
            }
        }
        let mut pass = 0;
        while pass < 4 && max_rank > 0 {
            let downward = (pass & 1) == 0;
            for s in 1..=max_rank {
                let row = &rows[if downward { s } else { max_rank - s }];
                let start = if downward { &up_start } else { &down_start };
                let list = if downward { &up } else { &down };
                for &i in row {
                    let mut sum = 0.0;
                    let mut count = 0;
                    for k in start[i]..start[i + 1] {
                        if lane_[list[k]] != lane_[i] {
                            continue;
                        }
                        sum += c[list[k]];
                        count += 1;
                    }
                    want[i] = if count > 0 { sum / count as f64 } else { c[i] };
                }
                for (from, to) in runs(row, &lane_) {
                    let lane = lane_[row[from]] as usize;
                    let mut floor = lane_start[lane] + pad[lane];
                    for k in from..=to {
                        let i = row[k];
                        c[i] = want[i].max(floor + size[i] / 2.0);
                        if k < to {
                            floor = c[i] + size[i] / 2.0 + gap(i, row[k + 1]);
                        }
                    }
                    let mut ceil = lane_start[lane + 1] - pad[lane];
                    for k in (from..=to).rev() {
                        let i = row[k];
                        c[i] = c[i].min(ceil - size[i] / 2.0);
                        if k > from {
                            ceil = c[i] - size[i] / 2.0 - gap(row[k - 1], i);
                        }
                    }
                }
            }
            pass += 1;
        }

        // Along the flow: one band per rank, after the lane titles.
        let mut band_start = vec![0.0; max_rank + 1];
        let mut band_end = vec![0.0; max_rank + 1];
        // The room after a rank is for the relations that turn there or are
        // only as long as they were asked to be. Where just plain handoffs
        // and straight lines pass, the ranks close up, as far as the label of
        // a handoff between them leaves it a line to sit on.
        let mut sep = vec![opt.rank_sep / step as f64; max_rank + 1];
        if plain.contains(&true) {
            let full = opt.rank_sep / step as f64;
            sep.fill(full / CLOSE);
            for k in 0..m {
                let e = &ledges[k];
                let a = if e.reversed { e.head } else { e.tail };
                let b = if e.reversed { e.tail } else { e.head };
                if plain[k] {
                    if e.label.h <= 0.0 || span[k] > step {
                        continue;
                    }
                    let room = full.min(
                        (e.label.h + 2.0 * TURN - depth[a].min(depth[b]) / 2.0) / span[k] as f64,
                    );
                    for r in rank_[a]..rank_[b] {
                        if room > sep[r as usize] {
                            sep[r as usize] = room;
                        }
                    }
                    continue;
                }
                let Some(d0) = first_dummy[k].filter(|_| span[k] > step * e.minlen) else {
                    for r in rank_[a]..rank_[b] {
                        sep[r as usize] = full;
                    }
                    continue;
                };
                let d0 = d0 as i32;
                for d in d0 - 1..d0 + span[k] - 1 {
                    let u = if d < d0 { a } else { d as usize };
                    let v = if d + 1 < d0 + span[k] - 1 {
                        (d + 1) as usize
                    } else {
                        b
                    };
                    if lane_[u] != lane_[v] || (c[u] - c[v]).abs() >= 0.5 {
                        sep[rank_[u] as usize] = full;
                    }
                }
            }
        }
        let mut at = head + self.end_pad;
        for r in 0..=max_rank {
            let mut deepest = 0.0;
            for &i in &rows[r] {
                if depth[i] > deepest {
                    deepest = depth[i];
                }
            }
            band_start[r] = at;
            band_end[r] = at + deepest;
            at += deepest + if r == max_rank { 0.0 } else { sep[r] };
        }
        let length = at + self.end_pad;
        let breadth_total = lane_start[lane_count];
        let router = Router {
            c: &c,
            size: &size,
            depth: &depth,
            rank: &rank_,
            band_start: &band_start,
            band_end: &band_end,
            real: real.len(),
            sideways,
            flipped: self.flipped,
            head,
            length,
            breadth_total,
            axis: if sideways { Axis::X } else { Axis::Y },
        };
        let axis = router.axis;

        for k in 0..real.len() {
            let node = &mut nodes[real[k]];
            let r = rank_[k] as usize;
            let a = (band_start[r] + band_end[r]) / 2.0;
            node.center = router.point(c[k], a);
        }
        for k in 0..lanes.len() {
            let lane = &mut nodes[lanes[k]];
            let middle = (lane_start[k] + lane_start[k + 1]) / 2.0;
            let size = lane_start[k + 1] - lane_start[k];
            lane.center = if sideways {
                Point::new(length / 2.0, middle)
            } else {
                Point::new(middle, length / 2.0)
            };
            lane.size = if sideways {
                Size::new(length, size)
            } else {
                Size::new(size, length)
            };
            lane.pad_top = head;
        }

        // Groups inside a lane are drawn around their members, deepest first.
        let mut depth_of = vec![0usize; n];
        let mut inner: Vec<usize> = Vec::new();
        for i in 0..n {
            if !nodes[i].is_group || self.lane_slot[i].is_some() || self.item_of[i].is_some() {
                continue;
            }
            let mut d = 0;
            let mut p = nodes[i].parent;
            while let Some(at) = p
                && d < MAX_HOPS
            {
                d += 1;
                p = nodes[at].parent;
            }
            depth_of[i] = d;
            inner.push(i);
        }
        inner.sort_by(|&a, &b| depth_of[b].cmp(&depth_of[a]).then(a.cmp(&b)));
        // The box around each group's members, as its least and greatest
        // corners.
        let mut bounds = vec![
            (
                Point::new(f64::INFINITY, f64::INFINITY),
                Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY)
            );
            n
        ];
        let grow = |bounds: &mut [(Point, Point)], g: usize, c: &CNode| {
            let (min, max) = &mut bounds[g];
            if c.center.x - c.size.w / 2.0 < min.x {
                min.x = c.center.x - c.size.w / 2.0;
            }
            if c.center.y - c.size.h / 2.0 < min.y {
                min.y = c.center.y - c.size.h / 2.0;
            }
            if c.center.x + c.size.w / 2.0 > max.x {
                max.x = c.center.x + c.size.w / 2.0;
            }
            if c.center.y + c.size.h / 2.0 > max.y {
                max.y = c.center.y + c.size.h / 2.0;
            }
        };
        // The group around a node inside a lane, if it is not the lane.
        let inner_group = |c: &CNode| c.parent.filter(|&p| self.lane_slot[p].is_none());
        for &i in real {
            if let Some(p) = inner_group(&nodes[i]) {
                grow(&mut bounds, p, &nodes[i]);
            }
        }
        for &g in &inner {
            let (min, max) = bounds[g];
            if min.x == f64::INFINITY {
                continue;
            }
            let c = &mut nodes[g];
            let w = (max.x - min.x + 2.0 * c.pad_x).max(c.min_w);
            c.center = Point::new(
                (min.x + max.x) / 2.0,
                (min.y - c.pad_top + max.y + c.pad_bottom) / 2.0,
            );
            c.size = Size::new(w, max.y - min.y + c.pad_top + c.pad_bottom);
            if let Some(p) = inner_group(&nodes[g]) {
                grow(&mut bounds, p, &nodes[g]);
            }
        }

        for e in edges.iter_mut() {
            e.route = Vec::new();
        }
        for k in 0..m {
            let e = &ledges[k];
            let a = if e.reversed { e.head } else { e.tail };
            let b = if e.reversed { e.tail } else { e.head };
            let (ra, rb) = (rank_[a] as usize, rank_[b] as usize);
            let mut pts = vec![Point::new(
                c[a],
                band_end[ra] - (band_end[ra] - band_start[ra] - depth[a]) / 2.0,
            )];
            if let Some(d0) = first_dummy[k] {
                for d in d0..d0 + span[k] as usize - 1 {
                    let r = rank_[d] as usize;
                    pts.push(Point::new(c[d], band_start[r]));
                    if band_end[r] > band_start[r] {
                        pts.push(Point::new(c[d], band_end[r]));
                    }
                }
            }
            pts.push(Point::new(
                c[b],
                band_start[rb] + (band_end[rb] - band_start[rb] - depth[b]) / 2.0,
            ));
            let mut route: Vec<RoutePoint> = pts
                .iter()
                .map(|&p| RoutePoint::new(router.place(p), axis))
                .collect();
            if e.reversed {
                route.reverse();
            }
            let out = &mut edges[self.edge_of[k]];
            out.label_at = match label_item[k] {
                Some(label) => {
                    let r = rank_[label] as usize;
                    router.point(c[label], (band_start[r] + band_end[r]) / 2.0)
                }
                None => route[0].at.midpoint(route[route.len() - 1].at),
            };
            out.route = route;
        }

        // An edge that starts or ends at a lane or a group runs straight to
        // its border.
        for e in edges.iter_mut() {
            if !e.route.is_empty() || e.src == e.dst {
                continue;
            }
            let a = &nodes[e.src];
            let b = &nodes[e.dst];
            let from = a.border_toward(b);
            let to = b.border_toward(a);
            let flat = if (to.x - from.x).abs() > (to.y - from.y).abs() {
                Axis::X
            } else {
                Axis::Y
            };
            e.route = vec![RoutePoint::new(from, flat), RoutePoint::new(to, flat)];
            e.label_at = from.midpoint(to);
        }

        // A handoff can leave towards the other lane, then arrive along the
        // flow (or vice versa). In particular, a decision's return branch must
        // not leave through its incoming side and retrace the path past the
        // preceding step. Try the two single-corner routes, keeping the
        // ranked route when a node or another label blocks them.
        let mut routed = vec![false; m];
        let route_points: usize = edges.iter().map(|edge| edge.route.len()).sum();
        let work = m * (real.len() + edges.len() + route_points);
        if work <= ROUTE_BUDGET {
            let mut labels: Vec<Point> = edges
                .iter()
                .map(|e| {
                    let at = e.label_at;
                    if sideways {
                        Point::new(at.y, router.flip(at.x))
                    } else {
                        Point::new(at.x, router.flip(at.y))
                    }
                })
                .collect();
            for (k, e) in ledges.iter().enumerate() {
                let ei = self.edge_of[k];
                let a = e.tail;
                let b = e.head;
                if lane_[a] == lane_[b] || (c[b] - c[a]).abs() < 1.0 {
                    continue;
                }
                let ac = c[a];
                let bc = c[b];
                let aa = router.amid(a);
                let ba = router.amid(b);
                let dc = (bc - ac).signum();
                let da = js::sign(ba - aa);
                let first = [
                    Point::new(ac + dc * size[a] / 2.0, aa),
                    Point::new(bc, aa),
                    Point::new(bc, ba - da * depth[b] / 2.0),
                ];
                if router.attempt(edges, &mut labels, &mut routed, e, k, ei, &first, 2.0) {
                    continue;
                }
                let second = [
                    Point::new(ac, aa + da * depth[a] / 2.0),
                    Point::new(ac, ba),
                    Point::new(bc - dc * size[b] / 2.0, ba),
                ];
                if router.attempt(edges, &mut labels, &mut routed, e, k, ei, &second, 2.0) {
                    continue;
                }
                // With both corners blocked, as for a return past the steps
                // between, the handoff goes around: out to the margin of a
                // lane, along it, and back in level with its end. It takes the
                // margin that crosses the fewest other relations and shares
                // the fewest ends with them, then the shortest way.
                if work > ROUTE_BUDGET / RELAY {
                    continue;
                }
                let mut around: Vec<([Point; 4], f64, usize)> = Vec::new();
                let pair = if lane_[a] < lane_[b] {
                    [lane_[b], lane_[a]]
                } else {
                    [lane_[a], lane_[b]]
                };
                for lane in pair {
                    let lane = lane as usize;
                    // A lane that keeps no margin yet has less room there, and
                    // the handoff stays inside it.
                    let inset = (pad[lane] - LEG).max((pad[lane] / 2.0).min(MARGIN - LEG));
                    for ch in [lane_start[lane] + inset, lane_start[lane + 1] - inset] {
                        if (ch - ac).abs() < size[a] / 2.0 + 4.0
                            || (ch - bc).abs() < size[b] / 2.0 + 4.0
                        {
                            continue;
                        }
                        let pts = [
                            Point::new(ac + (ch - ac).signum() * size[a] / 2.0, aa),
                            Point::new(ch, aa),
                            Point::new(ch, ba),
                            Point::new(bc + (ch - bc).signum() * size[b] / 2.0, ba),
                        ];
                        let ends = Router::meets(edges, router.place(pts[0]), ei)
                            + Router::meets(edges, router.place(pts[3]), ei);
                        let mut cost = ends as f64 * 1e6;
                        for pair in pts.windows(2) {
                            let p = router.place(pair[0]);
                            let q = router.place(pair[1]);
                            let crossed = Router::crossings(
                                edges,
                                Point::new(p.x.min(q.x), p.y.min(q.y)),
                                Point::new(p.x.max(q.x), p.y.max(q.y)),
                                ei,
                            );
                            cost += crossed as f64 * 1e6 + (q.x - p.x).abs() + (q.y - p.y).abs();
                        }
                        around.push((pts, cost, lane));
                    }
                }
                around.sort_by(|p, q| compare(p.1 - q.1));
                for (pts, _, lane) in &around {
                    if !router.attempt(edges, &mut labels, &mut routed, e, k, ei, pts, 1.0) {
                        continue;
                    }
                    used[*lane] = true;
                    break;
                }
            }
        }

        Placed {
            size: if sideways {
                Size::new(length, breadth_total)
            } else {
                Size::new(breadth_total, length)
            },
            routed,
            margins: used,
            work,
        }
    }
}

pub fn lane_layout(
    nodes: &mut [CNode],
    edges: &mut [CEdge],
    dir: Direction,
    opt: &LayeredOptions,
    lane_pad: Option<f64>,
) -> Size {
    let lane_pad = lane_pad.unwrap_or(LANE_PAD);
    let n = nodes.len();
    if n == 0 {
        return Size::default();
    }
    let sideways = !dir.is_vertical();
    let flipped = dir.is_reversed();

    // Lanes in the order they were written. The graph lists groups
    // last-declared first.
    let lanes: Vec<usize> = (0..n)
        .rev()
        .filter(|&i| nodes[i].parent.is_none() && nodes[i].is_group)
        .collect();
    let lane_count = lanes.len().max(1);
    let mut lane_slot = vec![None; n];
    for (k, &id) in lanes.iter().enumerate() {
        lane_slot[id] = Some(k);
    }
    let mut has_child = vec![false; n];
    for node in nodes.iter() {
        if let Some(p) = node.parent {
            has_child[p] = true;
        }
    }
    // Room a lane keeps around its items, so that the groups drawn around
    // them stay inside it.
    let mut lane_pads = vec![lane_pad; lane_count];
    let mut end_pad = lane_pad;

    // What gets ranked and placed: nodes, and groups that are empty and so
    // act as nodes.
    let mut item_of = vec![None; n];
    let mut real: Vec<usize> = Vec::new();
    for i in 0..n {
        if lane_slot[i].is_some() {
            continue;
        }
        if !nodes[i].is_group || !has_child[i] {
            item_of[i] = Some(real.len());
            real.push(i);
        }
    }
    // Sizes across the flow, then along it.
    let crosswise = |size: Size| if sideways { size.turned() } else { size };

    let mut lnodes: Vec<LNode> = real
        .iter()
        .map(|&i| LNode::new(crosswise(nodes[i].size), Kind::Node))
        .collect();
    let mut ledges: Vec<LEdge> = Vec::new();
    let mut edge_of: Vec<usize> = Vec::new();
    let mut step = 1;
    {
        let mut seen = HashSet::new();
        for (ei, e) in edges.iter().enumerate() {
            let (Some(a), Some(b)) = (item_of[e.src], item_of[e.dst]) else {
                continue;
            };
            if a == b {
                continue;
            }
            let key = a.min(b) * real.len() + a.max(b);
            if e.label.w > 0.0 || seen.contains(&key) {
                step = 2;
            }
            seen.insert(key);
            ledges.push(LEdge::new(a, b, e.minlen, crosswise(e.label)));
            edge_of.push(ei);
        }
    }
    orient(&lnodes, &mut ledges);
    rank(&mut lnodes, &ledges, step);

    let m = ledges.len();
    let item_lane: Vec<i32> = real
        .iter()
        .map(|&i| {
            let mut t = i;
            let mut room = lane_pad;
            let mut hops = 0;
            while let Some(p) = nodes[t].parent
                && hops < MAX_HOPS
            {
                t = p;
                if lane_slot[t].is_none() {
                    room += nodes[t]
                        .pad_x
                        .max(nodes[t].pad_top)
                        .max(nodes[t].pad_bottom);
                }
                hops += 1;
            }
            let lane = lane_slot[t].unwrap_or(0);
            if room > lane_pads[lane] {
                lane_pads[lane] = room;
            }
            if room > end_pad {
                end_pad = room;
            }
            lane as i32
        })
        .collect();

    let layout = Lanes {
        opt,
        sideways,
        flipped,
        lanes,
        lane_count,
        lane_slot,
        lane_pads,
        end_pad,
        item_of,
        real,
        lnodes,
        ledges,
        edge_of,
        item_lane,
        step,
    };

    let mut plain = vec![false; m];
    let mut margins = vec![false; lane_count];
    let mut placed = layout.place(nodes, edges, &plain, &margins);
    // A handoff that was given a simple route needs no room kept for it
    // through the ranks it passes, so the lanes are laid out again without
    // it. A handoff that then loses its route goes back to the ranked one.
    // Routing is most of the work, so a large graph is laid out once.
    let mut round = 0;
    while round < ROUNDS && placed.work <= ROUTE_BUDGET / RELAY {
        let next: Vec<bool> = if round == 0 {
            placed.routed.clone()
        } else {
            plain
                .iter()
                .zip(&placed.routed)
                .map(|(&on, &routed)| on && routed)
                .collect()
        };
        if next == plain && placed.margins == margins {
            break;
        }
        plain = next;
        margins = placed.margins.clone();
        placed = layout.place(nodes, edges, &plain, &margins);
        round += 1;
    }
    if plain
        .iter()
        .zip(&placed.routed)
        .any(|(&on, &routed)| on && !routed)
    {
        placed = layout.place(nodes, edges, &vec![false; m], &vec![false; lane_count]);
    }
    placed.size
}
