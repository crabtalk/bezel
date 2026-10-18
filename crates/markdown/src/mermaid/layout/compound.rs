//! Layout of a graph whose nodes may be nested in groups, each group laid out
//! with [`layered`] in its own direction.

use std::collections::HashMap;

use crate::mermaid::js;

use super::layered::{Kind, LEdge, LNode, LayeredOptions, layered, ledge, lnode, orient, rank};

const MAX_PORTS: usize = 40000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum Dir {
    #[default]
    TB,
    BT,
    LR,
    RL,
}

/// The direction a diagram names, or top to bottom for anything else.
pub fn direction(dir: Option<&str>) -> Dir {
    match dir {
        Some("BT") => Dir::BT,
        Some("LR") => Dir::LR,
        Some("RL") => Dir::RL,
        _ => Dir::TB,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CNode {
    pub w: f64,
    pub h: f64,
    pub parent: i32,
    pub is_group: bool,
    pub dir: Option<Dir>,
    pub pad_x: f64,
    pub pad_top: f64,
    pub pad_bottom: f64,
    pub min_w: f64,
    /// Declaration order; a group takes the smallest value among its members.
    pub seq: f64,
    /// How much of a side its edges may spread along, measured across the
    /// flow; 0 keeps them at its middle.
    pub span: f64,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CEdge {
    pub src: usize,
    pub dst: usize,
    pub minlen: i32,
    pub label_w: f64,
    pub label_h: f64,
    /// x, y, axis triples; axis is 0 where the edge runs vertically into the
    /// point, 1 horizontally.
    pub route: Vec<f64>,
    pub label_x: f64,
    pub label_y: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CompoundResult {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Side {
    Top = 0,
    Right = 1,
    Bottom = 2,
    Left = 3,
}

struct PortEdge {
    e: usize,
    inner: usize,
    out: bool,
    side: Side,
    px: f64,
    py: f64,
    route: Vec<f64>,
}

#[derive(Clone, Copy)]
struct LevelEdge {
    e: usize,
    a: usize,
    b: usize,
}

struct Level {
    id: i32,
    dir: Dir,
    items: Vec<usize>,
    edges: Vec<LevelEdge>,
    ports: Vec<PortEdge>,
    /// The ports of `ports` by `edge * 2`, plus 1 for the end an edge arrives
    /// at.
    port_of: HashMap<usize, usize>,
    routes: HashMap<usize, Vec<f64>>,
    x: f64,
    y: f64,
}

/// The levels of a layout, in the order they were made, and each by the id
/// of the group it lays out (-1 for the root).
struct Levels {
    list: Vec<Level>,
    of: HashMap<i32, usize>,
}

impl Levels {
    fn make(&mut self, id: i32, dir: Dir) -> usize {
        let at = self.list.len();
        self.list.push(Level {
            id,
            dir,
            items: Vec::new(),
            edges: Vec::new(),
            ports: Vec::new(),
            port_of: HashMap::new(),
            routes: HashMap::new(),
            x: 0.0,
            y: 0.0,
        });
        self.of.insert(id, at);
        at
    }

    fn of(&self, id: usize) -> usize {
        self.of[&(id as i32)]
    }

    /// The port in group `group` of the end of edge `e` that leaves (`out`)
    /// or arrives.
    fn port(&self, group: usize, e: usize, out: bool) -> &PortEdge {
        let level = &self.list[self.of(group)];
        &level.ports[level.port_of[&(e * 2 + if out { 0 } else { 1 })]]
    }

    fn port_mut(&mut self, group: usize, e: usize, out: bool) -> &mut PortEdge {
        let at = self.of(group);
        let level = &mut self.list[at];
        let index = level.port_of[&(e * 2 + if out { 0 } else { 1 })];
        &mut level.ports[index]
    }
}

pub fn shift_layout(nodes: &mut [CNode], edges: &mut [CEdge], ox: f64, oy: f64) {
    for c in nodes.iter_mut() {
        c.x += ox;
        c.y += oy;
    }
    for e in edges.iter_mut() {
        for k in (0..e.route.len()).step_by(3) {
            e.route[k] += ox;
            e.route[k + 1] += oy;
        }
        e.label_x += ox;
        e.label_y += oy;
    }
}

pub fn cnode(w: f64, h: f64, parent: i32) -> CNode {
    CNode {
        w,
        h,
        parent,
        is_group: false,
        dir: None,
        pad_x: 0.0,
        pad_top: 0.0,
        pad_bottom: 0.0,
        min_w: 0.0,
        seq: 0.0,
        span: 0.0,
        x: 0.0,
        y: 0.0,
    }
}

fn start_side(dir: Dir) -> Side {
    match dir {
        Dir::TB => Side::Top,
        Dir::BT => Side::Bottom,
        Dir::LR => Side::Left,
        Dir::RL => Side::Right,
    }
}

fn opposite(side: Side) -> Side {
    match side {
        Side::Top => Side::Bottom,
        Side::Right => Side::Left,
        Side::Bottom => Side::Top,
        Side::Left => Side::Right,
    }
}

fn parent(nodes: &[CNode], i: usize) -> usize {
    nodes[i].parent as usize
}

fn endpoint_below(item: usize, e: &CEdge, out: bool) -> bool {
    (if out { e.src } else { e.dst }) != item
}

/// The flat graph of one level: its items, then a port node for every edge
/// that crosses its border on the side the flow starts or ends at. `sized`
/// gives the nodes their sizes and the edges their labels and offsets.
fn build(
    nodes: &[CNode],
    edges: &[CEdge],
    levels: &Levels,
    local: &mut [usize],
    q: usize,
    sized: bool,
) -> (Vec<LNode>, Vec<LEdge>, Vec<i32>) {
    let level = &levels.list[q];
    let vertical = level.dir == Dir::TB || level.dir == Dir::BT;
    let start = start_side(level.dir);
    let mut lnodes: Vec<LNode> = Vec::new();
    let mut ledges: Vec<LEdge> = Vec::new();
    for (i, &item) in level.items.iter().enumerate() {
        local[item] = i;
        let node = &nodes[item];
        let mut ln = if sized {
            lnode(
                if vertical { node.w } else { node.h },
                if vertical { node.h } else { node.w },
                Kind::Node,
            )
        } else {
            lnode(0.0, 0.0, Kind::Node)
        };
        ln.span = if node.is_group {
            node.span.min(ln.w - 2.0 * node.pad_x)
        } else {
            node.span
        };
        lnodes.push(ln);
    }
    let held = |item: usize, ei: usize, out: bool| {
        nodes[item].is_group && endpoint_below(item, &edges[ei], out)
    };
    let offset = |item: usize, ei: usize, out: bool| -> f64 {
        if !sized || !nodes[item].is_group || !endpoint_below(item, &edges[ei], out) {
            return 0.0;
        }
        let port = levels.port(item, ei, out);
        if vertical {
            port.px - nodes[item].w / 2.0
        } else {
            port.py - nodes[item].h / 2.0
        }
    };
    for le in &level.edges {
        let e = &edges[le.e];
        let mut edge = ledge(local[le.a], local[le.b], e.minlen, 0.0, 0.0);
        if sized {
            edge.label_w = if vertical { e.label_w } else { e.label_h };
            edge.label_h = if vertical { e.label_h } else { e.label_w };
            edge.tail_dx = offset(le.a, le.e, true);
            edge.head_dx = offset(le.b, le.e, false);
            edge.tail_fixed = held(le.a, le.e, true);
            edge.head_fixed = held(le.b, le.e, false);
        }
        ledges.push(edge);
    }
    let mut dummies = vec![-1i32; level.ports.len()];
    for (i, port) in level.ports.iter().enumerate() {
        if port.side == start || port.side == opposite(start) {
            let d = lnodes.len();
            let kind = if port.side == start {
                Kind::StartPort
            } else {
                Kind::EndPort
            };
            lnodes.push(lnode(0.0, 0.0, kind));
            dummies[i] = d as i32;
            let mut edge = if port.out {
                ledge(local[port.inner], d, 1, 0.0, 0.0)
            } else {
                ledge(d, local[port.inner], 1, 0.0, 0.0)
            };
            if port.out {
                edge.tail_dx = offset(port.inner, port.e, true);
                edge.tail_fixed = held(port.inner, port.e, true);
            } else {
                edge.head_dx = offset(port.inner, port.e, false);
                edge.head_fixed = held(port.inner, port.e, false);
            }
            ledges.push(edge);
        } else {
            lnodes[local[port.inner]].pin = if port.side == Side::Top || port.side == Side::Left {
                -1.0
            } else {
                1.0
            };
        }
    }
    (lnodes, ledges, dummies)
}

/// Places the levels made from `from` on, and every level they hold, going
/// down the tree breadth first.
fn descend(
    nodes: &[CNode],
    children: &[Vec<usize>],
    levels: &mut Levels,
    depth: &mut [i32],
    placed: &mut [bool],
    from: usize,
) {
    let mut q = from;
    while q < levels.list.len() {
        let items = levels.list[q].items.clone();
        let id = levels.list[q].id;
        let dir = levels.list[q].dir;
        for item in items {
            placed[item] = true;
            depth[item] = if id < 0 { 0 } else { depth[id as usize] + 1 };
            if nodes[item].is_group {
                let child = levels.make(item as i32, nodes[item].dir.unwrap_or(dir));
                levels.list[child].items = children[item].clone();
            }
        }
        q += 1;
    }
}

fn append(route: &mut Vec<f64>, part: &[f64], ox: f64, oy: f64) {
    for i in (0..part.len()).step_by(3) {
        let x = part[i] + ox;
        let y = part[i + 1] + oy;
        let len = route.len();
        if len >= 3 && (route[len - 3] - x).abs() < 0.01 && (route[len - 2] - y).abs() < 0.01 {
            continue;
        }
        route.extend([x, y, part[i + 2]]);
    }
}

/// Lays out a graph whose nodes may be nested in groups. Each group is laid
/// out on its own, in its own direction, and then placed in its parent as a
/// single box. An edge that crosses a group's border passes through a port
/// on the side facing its other end.
pub fn compound_layout(
    nodes: &mut [CNode],
    edges: &mut [CEdge],
    root_dir: Dir,
    opt: &LayeredOptions,
) -> CompoundResult {
    let n = nodes.len();
    let mut levels = Levels {
        list: Vec::new(),
        of: HashMap::new(),
    };
    let root = levels.make(-1, root_dir);

    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut root_items: Vec<usize> = Vec::new();
    for i in 0..n {
        let p = nodes[i].parent;
        if p >= 0 && (p as usize) < n && p as usize != i && nodes[p as usize].is_group {
            children[p as usize].push(i);
        } else {
            nodes[i].parent = -1;
            root_items.push(i);
        }
    }
    levels.list[root].items = root_items;
    let mut depth = vec![0i32; n];
    let mut placed = vec![false; n];
    descend(nodes, &children, &mut levels, &mut depth, &mut placed, 0);
    // Groups that contain each other are unreachable from the root; lift them
    // out.
    for i in 0..n {
        if placed[i] {
            continue;
        }
        let former = parent(nodes, i);
        children[former].retain(|&c| c != i);
        nodes[i].parent = -1;
        levels.list[root].items.push(i);
        placed[i] = true;
        depth[i] = 0;
        if nodes[i].is_group {
            let from = levels.list.len();
            let child = levels.make(i as i32, nodes[i].dir.unwrap_or(root_dir));
            levels.list[child].items = children[i].clone();
            descend(nodes, &children, &mut levels, &mut depth, &mut placed, from);
        }
    }

    for q in (1..levels.list.len()).rev() {
        let level = &levels.list[q];
        let mut seq = f64::INFINITY;
        for &item in &level.items {
            if nodes[item].seq < seq {
                seq = nodes[item].seq;
            }
        }
        if seq != f64::INFINITY {
            nodes[level.id as usize].seq = seq;
        }
    }
    for level in &mut levels.list {
        level
            .items
            .sort_by(|&a, &b| js::compare(nodes[a].seq - nodes[b].seq).then(a.cmp(&b)));
    }

    // Every group border an edge crosses costs a port. Past the budget, an
    // edge is drawn between the groups that hold its ends, at the level they
    // share.
    let mut ports = 0;
    for e in edges.iter_mut() {
        let mut a = e.src;
        let mut b = e.dst;
        let mut crossed = 0;
        while depth[a] > depth[b] {
            a = parent(nodes, a);
            crossed += 1;
        }
        while depth[b] > depth[a] {
            b = parent(nodes, b);
            crossed += 1;
        }
        while a != b && nodes[a].parent != nodes[b].parent {
            a = parent(nodes, a);
            b = parent(nodes, b);
            crossed += 2;
        }
        if a == b {
            continue;
        }
        if ports + crossed > MAX_PORTS {
            e.src = a;
            e.dst = b;
        } else {
            ports += crossed;
        }
    }

    let mut loops: Vec<usize> = Vec::new();
    let mut stray: Vec<usize> = Vec::new();
    for (ei, e) in edges.iter_mut().enumerate() {
        e.route = Vec::new();
        if e.src == e.dst {
            loops.push(ei);
            continue;
        }
        let mut a = e.src;
        let mut b = e.dst;
        let mut a_path: Vec<usize> = Vec::new();
        let mut b_path: Vec<usize> = Vec::new();
        while depth[a] > depth[b] {
            a_path.push(a);
            a = parent(nodes, a);
        }
        while depth[b] > depth[a] {
            b_path.push(b);
            b = parent(nodes, b);
        }
        if a == b {
            stray.push(ei);
            continue;
        }
        while nodes[a].parent != nodes[b].parent {
            a_path.push(a);
            b_path.push(b);
            a = parent(nodes, a);
            b = parent(nodes, b);
        }
        let at = levels.of[&nodes[a].parent];
        levels.list[at].edges.push(LevelEdge { e: ei, a, b });
        for (path, out) in [(a_path, true), (b_path, false)] {
            for inner in path {
                let at = levels.of[&nodes[inner].parent];
                let level = &mut levels.list[at];
                level.ports.push(PortEdge {
                    e: ei,
                    inner,
                    out,
                    side: Side::Top,
                    px: 0.0,
                    py: 0.0,
                    route: Vec::new(),
                });
                level
                    .port_of
                    .insert(ei * 2 + if out { 0 } else { 1 }, level.ports.len() - 1);
            }
        }
    }

    let mut local = vec![0usize; n];

    // Top-down: rank each level to learn which side of a group every crossing
    // edge leaves from.
    for q in 0..levels.list.len() {
        for p in 0..levels.list[q].ports.len() {
            let port = &levels.list[q].ports[p];
            let (inner, e, out, side) = (port.inner, port.e, port.out, port.side);
            if nodes[inner].is_group && endpoint_below(inner, &edges[e], out) {
                levels.port_mut(inner, e, out).side = side;
            }
        }
        let level_edges = levels.list[q].edges.clone();
        if level_edges.is_empty() {
            continue;
        }
        let mut needed = false;
        for le in &level_edges {
            let e = &edges[le.e];
            if (nodes[le.a].is_group && e.src != le.a) || (nodes[le.b].is_group && e.dst != le.b) {
                needed = true;
            }
        }
        if !needed {
            continue;
        }
        let (mut lnodes, mut ledges, _) = build(nodes, edges, &levels, &mut local, q, false);
        orient(&lnodes, &mut ledges);
        rank(&mut lnodes, &ledges, 1);
        let start = start_side(levels.list[q].dir);
        for (i, le) in level_edges.iter().enumerate() {
            let e = &edges[le.e];
            let a_above = !ledges[i].reversed;
            if nodes[le.a].is_group && e.src != le.a {
                levels.port_mut(le.a, le.e, true).side =
                    if a_above { opposite(start) } else { start };
            }
            if nodes[le.b].is_group && e.dst != le.b {
                levels.port_mut(le.b, le.e, false).side =
                    if a_above { start } else { opposite(start) };
            }
        }
    }

    // Bottom-up: lay out each level now that the sizes of its groups are
    // known.
    let mut total = CompoundResult {
        width: 0.0,
        height: 0.0,
    };
    for q in (0..levels.list.len()).rev() {
        let id = levels.list[q].id;
        let group = (id >= 0).then_some(id as usize);
        let dir = levels.list[q].dir;
        let vertical = dir == Dir::TB || dir == Dir::BT;
        let flip = dir == Dir::BT || dir == Dir::RL;
        let (mut lnodes, mut ledges, dummies) = build(nodes, edges, &levels, &mut local, q, true);
        let size = layered(&mut lnodes, &mut ledges, opt);
        let content_w = if vertical { size.width } else { size.height };
        let content_h = if vertical { size.height } else { size.width };
        let pad_x = group.map_or(0.0, |g| nodes[g].pad_x);
        let pad_top = group.map_or(0.0, |g| nodes[g].pad_top);
        let box_w = (content_w + 2.0 * pad_x).max(group.map_or(0.0, |g| nodes[g].min_w));
        let box_h = content_h + pad_top + group.map_or(0.0, |g| nodes[g].pad_bottom);
        let off_x = (box_w - content_w) / 2.0;
        let off_y = pad_top;
        let tx = |lx: f64, ly: f64| -> f64 {
            off_x
                + if vertical {
                    lx
                } else if flip {
                    size.height - ly
                } else {
                    ly
                }
        };
        let ty = |lx: f64, ly: f64| -> f64 {
            off_y
                + if vertical {
                    if flip { size.height - ly } else { ly }
                } else {
                    lx
                }
        };
        let axis = if vertical { 0.0 } else { 1.0 };
        let convert = |points: &[f64]| -> Vec<f64> {
            let mut out = Vec::new();
            for i in (0..points.len()).step_by(2) {
                out.extend([
                    tx(points[i], points[i + 1]),
                    ty(points[i], points[i + 1]),
                    axis,
                ]);
            }
            out
        };

        for (i, &item) in levels.list[q].items.iter().enumerate() {
            nodes[item].x = tx(lnodes[i].x, lnodes[i].y);
            nodes[item].y = ty(lnodes[i].x, lnodes[i].y);
        }
        let level_edges = levels.list[q].edges.clone();
        for (i, le) in level_edges.iter().enumerate() {
            let edge = &ledges[i];
            levels.list[q].routes.insert(le.e, convert(&edge.points));
            let e = &mut edges[le.e];
            if e.label_w > 0.0 {
                e.label_x = tx(edge.label_x, edge.label_y);
                e.label_y = ty(edge.label_x, edge.label_y);
            }
        }
        let mut k = level_edges.len();
        for (i, &dummy) in dummies.iter().enumerate() {
            let (inner, e, out, side) = {
                let port = &levels.list[q].ports[i];
                (port.inner, port.e, port.out, port.side)
            };
            if dummy >= 0 {
                let mut route = convert(&ledges[k].points);
                k += 1;
                let at = if out { route.len() - 3 } else { 0 };
                let mut bx = route[at];
                let mut by = route[at + 1];
                match side {
                    Side::Top => by = 0.0,
                    Side::Bottom => by = box_h,
                    Side::Left => bx = 0.0,
                    Side::Right => bx = box_w,
                }
                if out {
                    route.extend([bx, by, axis]);
                } else {
                    route.splice(0..0, [bx, by, axis]);
                }
                let port = &mut levels.list[q].ports[i];
                port.px = bx;
                port.py = by;
                port.route = route;
                continue;
            }
            let item = &nodes[inner];
            let mut ix = item.x;
            let mut iy = item.y;
            if item.is_group && endpoint_below(inner, &edges[e], out) {
                let below = levels.port(inner, e, out);
                ix += below.px - item.w / 2.0;
                iy += below.py - item.h / 2.0;
            } else {
                match side {
                    Side::Top => iy -= item.h / 2.0,
                    Side::Bottom => iy += item.h / 2.0,
                    Side::Left => ix -= item.w / 2.0,
                    Side::Right => ix += item.w / 2.0,
                }
            }
            let mut bx = ix;
            let mut by = iy;
            match side {
                Side::Top => by = 0.0,
                Side::Bottom => by = box_h,
                Side::Left => bx = 0.0,
                Side::Right => bx = box_w,
            }
            let a = if side == Side::Top || side == Side::Bottom {
                0.0
            } else {
                1.0
            };
            let port = &mut levels.list[q].ports[i];
            port.px = bx;
            port.py = by;
            port.route = if out {
                vec![ix, iy, a, bx, by, a]
            } else {
                vec![bx, by, a, ix, iy, a]
            };
        }

        if let Some(g) = group {
            nodes[g].w = box_w;
            nodes[g].h = box_h;
        } else {
            total = CompoundResult {
                width: box_w,
                height: box_h,
            };
        }
    }

    // Top-down again: turn positions inside each box into diagram
    // coordinates.
    for q in 0..levels.list.len() {
        let ox = levels.list[q].x;
        let oy = levels.list[q].y;
        for i in 0..levels.list[q].items.len() {
            let item = levels.list[q].items[i];
            let node = &mut nodes[item];
            node.x += ox;
            node.y += oy;
            if node.is_group {
                let (x, y) = (node.x - node.w / 2.0, node.y - node.h / 2.0);
                let child = levels.of(item);
                levels.list[child].x = x;
                levels.list[child].y = y;
            }
        }
    }

    for level in &levels.list {
        for le in &level.edges {
            let e = &edges[le.e];
            let mut route: Vec<f64> = Vec::new();
            let mut outward: Vec<usize> = Vec::new();
            let mut item = e.src;
            while nodes[item].parent != level.id {
                outward.push(levels.of[&nodes[item].parent]);
                item = parent(nodes, item);
            }
            for &lv in &outward {
                let lv = &levels.list[lv];
                append(
                    &mut route,
                    &lv.ports[lv.port_of[&(le.e * 2)]].route,
                    lv.x,
                    lv.y,
                );
            }
            append(&mut route, &level.routes[&le.e], level.x, level.y);
            let mut inward: Vec<usize> = Vec::new();
            let mut item = e.dst;
            while nodes[item].parent != level.id {
                inward.push(levels.of[&nodes[item].parent]);
                item = parent(nodes, item);
            }
            for &lv in inward.iter().rev() {
                let lv = &levels.list[lv];
                append(
                    &mut route,
                    &lv.ports[lv.port_of[&(le.e * 2 + 1)]].route,
                    lv.x,
                    lv.y,
                );
            }
            let e = &mut edges[le.e];
            e.route = route;
            if e.label_w > 0.0 {
                e.label_x += level.x;
                e.label_y += level.y;
            }
        }
    }

    for ei in stray {
        let (ax, ay) = (nodes[edges[ei].src].x, nodes[edges[ei].src].y);
        let (bx, by) = (nodes[edges[ei].dst].x, nodes[edges[ei].dst].y);
        let e = &mut edges[ei];
        e.route = vec![ax, ay, 0.0, bx, by, 0.0];
        e.label_x = (ax + bx) / 2.0;
        e.label_y = (ay + by) / 2.0;
    }
    for ei in loops {
        let node = &nodes[edges[ei].src];
        let (x, y, w) = (node.x, node.y, node.w);
        let e = &mut edges[ei];
        e.label_x = x + w / 2.0 + 28.0 + e.label_w / 2.0;
        e.label_y = y;
    }

    total
}
