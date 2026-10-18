//! Layout for swimlanes. Every top-level group is a lane: a band that runs
//! the length of the diagram along the flow. Ranks are shared by all lanes, so
//! a step sits level with the steps that happen at the same time in other
//! lanes, and each node stays inside its own lane.

use std::collections::HashSet;

use crate::mermaid::js::{self, compare};

use super::{
    compound::{CEdge, CNode, CompoundResult, Dir},
    layered::{Kind, LEdge, LNode, LayeredOptions, ledge, lnode, orient, rank},
};

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
    width: f64,
    height: f64,
    routed: Vec<u8>,
    margins: Vec<u8>,
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
    lane_slot: Vec<i32>,
    lane_pads: Vec<f64>,
    end_pad: f64,
    item_of: Vec<i32>,
    real: Vec<usize>,
    lnodes: Vec<LNode>,
    ledges: Vec<LEdge>,
    edge_of: Vec<usize>,
    item_lane: Vec<i32>,
    step: i32,
}

/// Where the items of one placing are, for routing handoffs.
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
    axis: f64,
}

impl Router<'_> {
    fn flip(&self, a: f64) -> f64 {
        if self.flipped {
            self.head + self.length - a
        } else {
            a
        }
    }

    fn px(&self, c: f64, a: f64) -> f64 {
        if self.sideways { self.flip(a) } else { c }
    }

    fn py(&self, c: f64, a: f64) -> f64 {
        if self.sideways { c } else { self.flip(a) }
    }

    fn amid(&self, i: usize) -> f64 {
        let r = self.rank[i] as usize;
        (self.band_start[r] + self.band_end[r]) / 2.0
    }

    #[allow(clippy::too_many_arguments)]
    fn overlaps(c0: f64, a0: f64, c1: f64, a1: f64, lc: f64, la: f64, cw: f64, ah: f64) -> bool {
        c1 > lc - cw / 2.0 && c0 < lc + cw / 2.0 && a1 > la - ah / 2.0 && a0 < la + ah / 2.0
    }

    #[allow(clippy::too_many_arguments)]
    fn clear(
        &self,
        edges: &[CEdge],
        labels: &[[f64; 2]],
        c0: f64,
        a0: f64,
        c1: f64,
        a1: f64,
        src: i32,
        dst: i32,
        ei: usize,
    ) -> bool {
        for i in 0..self.real {
            if i as i32 != src
                && i as i32 != dst
                && Self::overlaps(
                    c0,
                    a0,
                    c1,
                    a1,
                    self.c[i],
                    self.amid(i),
                    self.size[i] + 8.0,
                    self.depth[i] + 8.0,
                )
            {
                return false;
            }
        }
        for (i, e) in edges.iter().enumerate() {
            // A step's own loop keeps its label inside the step's box, and
            // has no place here.
            if i != ei
                && e.label_w > 0.0
                && e.src != e.dst
                && Self::overlaps(
                    c0,
                    a0,
                    c1,
                    a1,
                    labels[i][0],
                    labels[i][1],
                    if self.sideways { e.label_h } else { e.label_w },
                    if self.sideways { e.label_w } else { e.label_h },
                )
            {
                return false;
            }
        }
        true
    }

    /// How many other relations have a stretch inside a box. A stretch's
    /// whole box counts, including its curve.
    fn crossings(edges: &[CEdge], x0: f64, y0: f64, x1: f64, y1: f64, ei: usize) -> usize {
        let mut count = 0;
        for (i, edge) in edges.iter().enumerate() {
            if i == ei {
                continue;
            }
            let route = &edge.route;
            for j in (3..route.len()).step_by(3) {
                if route[j - 3].max(route[j]) > x0
                    && route[j - 3].min(route[j]) < x1
                    && route[j - 2].max(route[j + 1]) > y0
                    && route[j - 2].min(route[j + 1]) < y1
                {
                    count += 1;
                    break;
                }
            }
        }
        count
    }

    fn meets(edges: &[CEdge], x: f64, y: f64, ei: usize) -> usize {
        let mut count = 0;
        for (i, edge) in edges.iter().enumerate() {
            let route = &edge.route;
            if i == ei || route.is_empty() {
                continue;
            }
            let last = route.len() - 3;
            if ((route[0] - x).abs() < 1.0 && (route[1] - y).abs() < 1.0)
                || ((route[last] - x).abs() < 1.0 && (route[last + 1] - y).abs() < 1.0)
            {
                count += 1;
            }
        }
        count
    }

    /// Two relations drawn along one line read as a single path. Those that
    /// leave the same step or arrive at the same step may share a trunk; any
    /// other pair must not.
    fn retraces(edges: &[CEdge], x0: f64, y0: f64, x1: f64, y1: f64, ei: usize) -> bool {
        let upright = (x1 - x0).abs() < 0.5;
        let lo = if upright { y0.min(y1) } else { x0.min(x1) };
        let hi = if upright { y0.max(y1) } else { x0.max(x1) };
        let line = if upright { x0 } else { y0 };
        let (across, along) = if upright { (0, 1) } else { (1, 0) };
        for (i, other) in edges.iter().enumerate() {
            if i == ei || other.src == edges[ei].src || other.dst == edges[ei].dst {
                continue;
            }
            let route = &other.route;
            for j in (3..route.len()).step_by(3) {
                let u0 = route[j - 3 + across];
                let u1 = route[j + across];
                if (u0 - line).abs() >= NEAR || (u1 - line).abs() >= NEAR {
                    continue;
                }
                let v0 = route[j - 3 + along];
                let v1 = route[j + along];
                if hi.min(v0.max(v1)) - lo.max(v0.min(v1)) > 1.0 {
                    return true;
                }
            }
        }
        false
    }

    /// Tries the route through `pts` (across, along pairs) for relation `k`,
    /// and takes it when it keeps `margin` clear of what is in its way.
    #[allow(clippy::too_many_arguments)]
    fn attempt(
        &self,
        edges: &mut [CEdge],
        labels: &mut [[f64; 2]],
        routed: &mut [u8],
        e: &LEdge,
        k: usize,
        ei: usize,
        pts: &[f64],
        margin: f64,
    ) -> bool {
        let (a, b) = (e.tail as i32, e.head as i32);
        for p in (2..pts.len()).step_by(2) {
            if !self.clear(
                edges,
                labels,
                pts[p - 2].min(pts[p]) - margin,
                pts[p - 1].min(pts[p + 1]) - margin,
                pts[p - 2].max(pts[p]) + margin,
                pts[p - 1].max(pts[p + 1]) + margin,
                a,
                b,
                ei,
            ) {
                return false;
            }
            if Self::retraces(
                edges,
                self.px(pts[p - 2], pts[p - 1]),
                self.py(pts[p - 2], pts[p - 1]),
                self.px(pts[p], pts[p + 1]),
                self.py(pts[p], pts[p + 1]),
                ei,
            ) {
                return false;
            }
        }
        let (mut lc, mut la, mut longest) = (0.0, 0.0, -1.0);
        let (out_w, out_h) = (edges[ei].label_w, edges[ei].label_h);
        if out_w > 0.0 {
            let cw = e.label_w;
            let ah = e.label_h;
            for p in (2..pts.len()).step_by(2) {
                let cc = (pts[p - 2] + pts[p]) / 2.0;
                let ca = (pts[p - 1] + pts[p + 1]) / 2.0;
                let across = (pts[p] - pts[p - 2]).abs();
                let along = (pts[p + 1] - pts[p - 1]).abs();
                let run = across + along;
                if run <= longest
                    || (if across > 0.0 {
                        across < cw + 8.0
                    } else {
                        along < ah + 8.0
                    })
                {
                    continue;
                }
                if cc - cw / 2.0 < 0.0
                    || cc + cw / 2.0 > self.breadth_total
                    || ca - ah / 2.0 < 0.0
                    || ca + ah / 2.0 > self.length
                {
                    continue;
                }
                // Include the ends here: the label must clear both shapes it
                // connects.
                if !self.clear(
                    edges,
                    labels,
                    cc - cw / 2.0,
                    ca - ah / 2.0,
                    cc + cw / 2.0,
                    ca + ah / 2.0,
                    -1,
                    -1,
                    ei,
                ) {
                    continue;
                }
                // A label moved onto a handoff must not cover a return path
                // that keeps its ranked route. A segment's whole box is kept
                // clear, including its curve.
                let lx = self.px(cc, ca);
                let ly = self.py(cc, ca);
                if Self::crossings(
                    edges,
                    lx - out_w / 2.0,
                    ly - out_h / 2.0,
                    lx + out_w / 2.0,
                    ly + out_h / 2.0,
                    ei,
                ) > 0
                {
                    continue;
                }
                lc = cc;
                la = ca;
                longest = run;
            }
            if longest < 0.0 {
                return false;
            }
        }
        // A point short of each corner and one past it keep the turn close to
        // the corner, so that the line stays under its label and the handoff
        // takes little room.
        let mut way = vec![pts[0], pts[1]];
        for p in (2..pts.len()).step_by(2) {
            let dx = pts[p] - pts[p - 2];
            let dy = pts[p + 1] - pts[p - 1];
            let run = dx.abs() + dy.abs();
            let first = if p > 2 { TURN } else { 0.0 };
            let last = if p + 2 < pts.len() { TURN } else { 0.0 };
            if run > first + last + 1.0 {
                if first != 0.0 {
                    way.extend([
                        pts[p - 2] + (dx / run) * first,
                        pts[p - 1] + (dy / run) * first,
                    ]);
                }
                if last != 0.0 {
                    way.extend([pts[p] - (dx / run) * last, pts[p + 1] - (dy / run) * last]);
                }
            }
            way.extend([pts[p], pts[p + 1]]);
        }
        let mut route = Vec::new();
        for p in (0..way.len()).step_by(2) {
            let crosswise = if p == 0 {
                way[0] != way[2]
            } else {
                way[p] != way[p - 2]
            };
            route.extend([
                self.px(way[p], way[p + 1]),
                self.py(way[p], way[p + 1]),
                if crosswise {
                    1.0 - self.axis
                } else {
                    self.axis
                },
            ]);
        }
        let out = &mut edges[ei];
        out.route = route;
        if out.label_w > 0.0 {
            out.label_x = self.px(lc, la);
            out.label_y = self.py(lc, la);
            labels[ei] = [lc, la];
        }
        routed[k] = 1;
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
        plain: &[u8],
        margins: &[u8],
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
                if margins[lane] != 0 {
                    room.max(MARGIN)
                } else {
                    room
                }
            })
            .collect();
        let mut used = vec![0u8; lane_count];
        // Items: the real ones, then a dummy for every rank an edge passes
        // through.
        let mut dummies = 0i64;
        let mut span = vec![0i32; m];
        for k in 0..m {
            span[k] = (lnodes[ledges[k].head].rank - lnodes[ledges[k].tail].rank).abs();
            if plain[k] == 0 {
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
            size[k] = lnodes[k].w;
            depth[k] = lnodes[k].h;
            if rank_[k] > max_rank {
                max_rank = rank_[k];
            }
        }
        let max_rank = max_rank as usize;
        let mut first_dummy = vec![-1i32; m];
        let mut label_item = vec![-1i32; m];
        let mut links: Vec<usize> = Vec::new();
        {
            let mut next = real.len();
            for k in 0..m {
                let e = &ledges[k];
                let a = if e.reversed { e.head } else { e.tail };
                let b = if e.reversed { e.tail } else { e.head };
                if direct || span[k] < 2 || plain[k] != 0 {
                    links.extend([a, b]);
                    continue;
                }
                let ra = rank_[a];
                let rb = rank_[b];
                let mut label_rank = -1;
                if e.label_w > 0.0 {
                    label_rank = ra + ((rb - ra) >> 1);
                    if step == 2 && (label_rank & 1) == 0 {
                        label_rank -= 1;
                    }
                }
                first_dummy[k] = next as i32;
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
                        size[d] = e.label_w;
                        depth[d] = e.label_h;
                        label_item[k] = d as i32;
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
        if plain.contains(&1) {
            let full = opt.rank_sep / step as f64;
            sep.fill(full / CLOSE);
            for k in 0..m {
                let e = &ledges[k];
                let a = if e.reversed { e.head } else { e.tail };
                let b = if e.reversed { e.tail } else { e.head };
                if plain[k] != 0 {
                    if e.label_h <= 0.0 || span[k] > step {
                        continue;
                    }
                    let room = full.min(
                        (e.label_h + 2.0 * TURN - depth[a].min(depth[b]) / 2.0) / span[k] as f64,
                    );
                    for r in rank_[a]..rank_[b] {
                        if room > sep[r as usize] {
                            sep[r as usize] = room;
                        }
                    }
                    continue;
                }
                let d0 = first_dummy[k];
                if d0 < 0 || span[k] <= step * e.minlen {
                    for r in rank_[a]..rank_[b] {
                        sep[r as usize] = full;
                    }
                    continue;
                }
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
            axis: if sideways { 1.0 } else { 0.0 },
        };
        let axis = router.axis;

        for k in 0..real.len() {
            let node = &mut nodes[real[k]];
            let r = rank_[k] as usize;
            let a = (band_start[r] + band_end[r]) / 2.0;
            node.x = router.px(c[k], a);
            node.y = router.py(c[k], a);
        }
        for k in 0..lanes.len() {
            let lane = &mut nodes[lanes[k]];
            let middle = (lane_start[k] + lane_start[k + 1]) / 2.0;
            let size = lane_start[k + 1] - lane_start[k];
            lane.x = if sideways { length / 2.0 } else { middle };
            lane.y = if sideways { middle } else { length / 2.0 };
            lane.w = if sideways { length } else { size };
            lane.h = if sideways { size } else { length };
            lane.pad_top = head;
        }

        // Groups inside a lane are drawn around their members, deepest first.
        let mut depth_of = vec![0usize; n];
        let mut inner: Vec<usize> = Vec::new();
        for i in 0..n {
            if !nodes[i].is_group || self.lane_slot[i] >= 0 || self.item_of[i] >= 0 {
                continue;
            }
            let mut d = 0;
            let mut p = nodes[i].parent;
            while p >= 0 && d < MAX_HOPS {
                d += 1;
                p = nodes[p as usize].parent;
            }
            depth_of[i] = d;
            inner.push(i);
        }
        inner.sort_by(|&a, &b| depth_of[b].cmp(&depth_of[a]).then(a.cmp(&b)));
        let mut bounds = vec![0.0; n * 4];
        for i in 0..n {
            bounds[i * 4] = f64::INFINITY;
            bounds[i * 4 + 1] = f64::INFINITY;
            bounds[i * 4 + 2] = f64::NEG_INFINITY;
            bounds[i * 4 + 3] = f64::NEG_INFINITY;
        }
        let grow = |bounds: &mut [f64], g: usize, c: &CNode| {
            if c.x - c.w / 2.0 < bounds[g * 4] {
                bounds[g * 4] = c.x - c.w / 2.0;
            }
            if c.y - c.h / 2.0 < bounds[g * 4 + 1] {
                bounds[g * 4 + 1] = c.y - c.h / 2.0;
            }
            if c.x + c.w / 2.0 > bounds[g * 4 + 2] {
                bounds[g * 4 + 2] = c.x + c.w / 2.0;
            }
            if c.y + c.h / 2.0 > bounds[g * 4 + 3] {
                bounds[g * 4 + 3] = c.y + c.h / 2.0;
            }
        };
        for &i in real {
            let p = nodes[i].parent;
            if p >= 0 && self.lane_slot[p as usize] < 0 {
                grow(&mut bounds, p as usize, &nodes[i]);
            }
        }
        for &g in &inner {
            if bounds[g * 4] == f64::INFINITY {
                continue;
            }
            let c = &mut nodes[g];
            let w = (bounds[g * 4 + 2] - bounds[g * 4] + 2.0 * c.pad_x).max(c.min_w);
            c.x = (bounds[g * 4] + bounds[g * 4 + 2]) / 2.0;
            c.w = w;
            c.y = (bounds[g * 4 + 1] - c.pad_top + bounds[g * 4 + 3] + c.pad_bottom) / 2.0;
            c.h = bounds[g * 4 + 3] - bounds[g * 4 + 1] + c.pad_top + c.pad_bottom;
            let p = c.parent;
            if p >= 0 && self.lane_slot[p as usize] < 0 {
                grow(&mut bounds, p as usize, &nodes[g]);
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
            let mut pts: Vec<f64> = vec![
                c[a],
                band_end[ra] - (band_end[ra] - band_start[ra] - depth[a]) / 2.0,
            ];
            let d0 = first_dummy[k];
            if d0 >= 0 {
                for d in d0..d0 + span[k] - 1 {
                    let d = d as usize;
                    let r = rank_[d] as usize;
                    pts.extend([c[d], band_start[r]]);
                    if band_end[r] > band_start[r] {
                        pts.extend([c[d], band_end[r]]);
                    }
                }
            }
            pts.extend([
                c[b],
                band_start[rb] + (band_end[rb] - band_start[rb] - depth[b]) / 2.0,
            ]);
            let mut route: Vec<f64> = Vec::new();
            for p in (0..pts.len()).step_by(2) {
                route.extend([
                    router.px(pts[p], pts[p + 1]),
                    router.py(pts[p], pts[p + 1]),
                    axis,
                ]);
            }
            if e.reversed {
                let mut i = 0;
                let mut j = route.len() - 3;
                while i < j {
                    for t in 0..2 {
                        route.swap(i + t, j + t);
                    }
                    i += 3;
                    j -= 3;
                }
            }
            let out = &mut edges[self.edge_of[k]];
            let label = label_item[k];
            if label >= 0 {
                let label = label as usize;
                let r = rank_[label] as usize;
                let mid = (band_start[r] + band_end[r]) / 2.0;
                out.label_x = router.px(c[label], mid);
                out.label_y = router.py(c[label], mid);
            } else {
                out.label_x = (route[0] + route[route.len() - 3]) / 2.0;
                out.label_y = (route[1] + route[route.len() - 2]) / 2.0;
            }
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
            let (ax, ay) = border(a, b);
            let (bx, by) = border(b, a);
            let flat = if (bx - ax).abs() > (by - ay).abs() {
                1.0
            } else {
                0.0
            };
            e.route = vec![ax, ay, flat, bx, by, flat];
            e.label_x = (ax + bx) / 2.0;
            e.label_y = (ay + by) / 2.0;
        }

        // A handoff can leave towards the other lane, then arrive along the
        // flow (or vice versa). In particular, a decision's return branch must
        // not leave through its incoming side and retrace the path past the
        // preceding step. Try the two single-corner routes, keeping the
        // ranked route when a node or another label blocks them.
        let mut routed = vec![0u8; m];
        let route_points: usize = edges.iter().map(|edge| edge.route.len() / 3).sum();
        let work = m * (real.len() + edges.len() + route_points);
        if work <= ROUTE_BUDGET {
            let mut labels: Vec<[f64; 2]> = edges
                .iter()
                .map(|e| {
                    [
                        if sideways { e.label_y } else { e.label_x },
                        router.flip(if sideways { e.label_x } else { e.label_y }),
                    ]
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
                let dc = js::sign(bc - ac);
                let da = js::sign(ba - aa);
                let first = [
                    ac + dc * size[a] / 2.0,
                    aa,
                    bc,
                    aa,
                    bc,
                    ba - da * depth[b] / 2.0,
                ];
                if router.attempt(edges, &mut labels, &mut routed, e, k, ei, &first, 2.0) {
                    continue;
                }
                let second = [
                    ac,
                    aa + da * depth[a] / 2.0,
                    ac,
                    ba,
                    bc - dc * size[b] / 2.0,
                    ba,
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
                let mut around: Vec<(Vec<f64>, f64, usize)> = Vec::new();
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
                        let pts = vec![
                            ac + js::sign(ch - ac) * size[a] / 2.0,
                            aa,
                            ch,
                            aa,
                            ch,
                            ba,
                            bc + js::sign(ch - bc) * size[b] / 2.0,
                            ba,
                        ];
                        let ends = Router::meets(
                            edges,
                            router.px(pts[0], pts[1]),
                            router.py(pts[0], pts[1]),
                            ei,
                        ) + Router::meets(
                            edges,
                            router.px(pts[6], pts[7]),
                            router.py(pts[6], pts[7]),
                            ei,
                        );
                        let mut cost = ends as f64 * 1e6;
                        for p in (2..pts.len()).step_by(2) {
                            let x0 = router.px(pts[p - 2], pts[p - 1]);
                            let y0 = router.py(pts[p - 2], pts[p - 1]);
                            let x1 = router.px(pts[p], pts[p + 1]);
                            let y1 = router.py(pts[p], pts[p + 1]);
                            let crossed = Router::crossings(
                                edges,
                                x0.min(x1),
                                y0.min(y1),
                                x0.max(x1),
                                y0.max(y1),
                                ei,
                            );
                            cost += crossed as f64 * 1e6 + (x1 - x0).abs() + (y1 - y0).abs();
                        }
                        around.push((pts, cost, lane));
                    }
                }
                around.sort_by(|p, q| compare(p.1 - q.1));
                for (pts, _, lane) in &around {
                    if !router.attempt(edges, &mut labels, &mut routed, e, k, ei, pts, 1.0) {
                        continue;
                    }
                    used[*lane] = 1;
                    break;
                }
            }
        }

        Placed {
            width: if sideways { length } else { breadth_total },
            height: if sideways { breadth_total } else { length },
            routed,
            margins: used,
            work,
        }
    }
}

pub fn lane_layout(
    nodes: &mut [CNode],
    edges: &mut [CEdge],
    dir: Dir,
    opt: &LayeredOptions,
    lane_pad: Option<f64>,
) -> CompoundResult {
    let lane_pad = lane_pad.unwrap_or(LANE_PAD);
    let n = nodes.len();
    if n == 0 {
        return CompoundResult {
            width: 0.0,
            height: 0.0,
        };
    }
    let sideways = dir == Dir::LR || dir == Dir::RL;
    let flipped = dir == Dir::BT || dir == Dir::RL;

    // Lanes in the order they were written. The graph lists groups
    // last-declared first.
    let lanes: Vec<usize> = (0..n)
        .rev()
        .filter(|&i| nodes[i].parent < 0 && nodes[i].is_group)
        .collect();
    let lane_count = lanes.len().max(1);
    let mut lane_slot = vec![-1i32; n];
    for (k, &id) in lanes.iter().enumerate() {
        lane_slot[id] = k as i32;
    }
    let mut has_child = vec![false; n];
    for node in nodes.iter() {
        if node.parent >= 0 {
            has_child[node.parent as usize] = true;
        }
    }
    // Room a lane keeps around its items, so that the groups drawn around
    // them stay inside it.
    let mut lane_pads = vec![lane_pad; lane_count];
    let mut end_pad = lane_pad;

    // What gets ranked and placed: nodes, and groups that are empty and so
    // act as nodes.
    let mut item_of = vec![-1i32; n];
    let mut real: Vec<usize> = Vec::new();
    for i in 0..n {
        if lane_slot[i] >= 0 {
            continue;
        }
        if !nodes[i].is_group || !has_child[i] {
            item_of[i] = real.len() as i32;
            real.push(i);
        }
    }
    let along = |c: &CNode| if sideways { c.w } else { c.h };
    let cross = |c: &CNode| if sideways { c.h } else { c.w };

    let mut lnodes: Vec<LNode> = real
        .iter()
        .map(|&i| lnode(cross(&nodes[i]), along(&nodes[i]), Kind::Node))
        .collect();
    let mut ledges: Vec<LEdge> = Vec::new();
    let mut edge_of: Vec<usize> = Vec::new();
    let mut step = 1;
    {
        let mut seen = HashSet::new();
        for (ei, e) in edges.iter().enumerate() {
            let a = item_of[e.src];
            let b = item_of[e.dst];
            if a < 0 || b < 0 || a == b {
                continue;
            }
            let (a, b) = (a as usize, b as usize);
            let key = a.min(b) * real.len() + a.max(b);
            if e.label_w > 0.0 || seen.contains(&key) {
                step = 2;
            }
            seen.insert(key);
            ledges.push(ledge(
                a,
                b,
                e.minlen,
                if sideways { e.label_h } else { e.label_w },
                if sideways { e.label_w } else { e.label_h },
            ));
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
            while nodes[t].parent >= 0 && hops < MAX_HOPS {
                t = nodes[t].parent as usize;
                if lane_slot[t] < 0 {
                    room += nodes[t]
                        .pad_x
                        .max(nodes[t].pad_top)
                        .max(nodes[t].pad_bottom);
                }
                hops += 1;
            }
            let lane = 0.max(lane_slot[t]) as usize;
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

    let mut plain = vec![0u8; m];
    let mut margins = vec![0u8; lane_count];
    let mut placed = layout.place(nodes, edges, &plain, &margins);
    // A handoff that was given a simple route needs no room kept for it
    // through the ranks it passes, so the lanes are laid out again without
    // it. A handoff that then loses its route goes back to the ranked one.
    // Routing is most of the work, so a large graph is laid out once.
    let mut round = 0;
    while round < ROUNDS && placed.work <= ROUTE_BUDGET / RELAY {
        let next: Vec<u8> = if round == 0 {
            placed.routed.clone()
        } else {
            plain
                .iter()
                .zip(&placed.routed)
                .map(|(&on, &routed)| on & routed)
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
        .any(|(&on, &routed)| on != 0 && routed == 0)
    {
        placed = layout.place(nodes, edges, &vec![0u8; m], &vec![0u8; lane_count]);
    }
    CompoundResult {
        width: placed.width,
        height: placed.height,
    }
}

/// The point where the line from one box's center to another's leaves the
/// first box. When the other center is inside the box, the point is on the
/// nearest side instead.
fn border(from: &CNode, to: &CNode) -> (f64, f64) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let hw = (from.w / 2.0).max(0.5);
    let hh = (from.h / 2.0).max(0.5);
    if dx.abs() < hw && dy.abs() < hh {
        let to_side = hw - dx.abs();
        let to_end = hh - dy.abs();
        return if to_side < to_end {
            (from.x + if dx < 0.0 { -hw } else { hw }, to.y)
        } else {
            (to.x, from.y + if dy < 0.0 { -hh } else { hh })
        };
    }
    let scale = (if dx != 0.0 {
        hw / dx.abs()
    } else {
        f64::INFINITY
    })
    .min(if dy != 0.0 {
        hh / dy.abs()
    } else {
        f64::INFINITY
    });
    (from.x + dx * scale, from.y + dy * scale)
}
