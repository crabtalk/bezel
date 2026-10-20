//! Layout of a graph whose nodes may be nested in groups, each group laid out
//! with [`layered`] in its own direction.

use std::collections::HashMap;

use crate::{
    geom::{Axis, Direction, Point, RoutePoint, Side, Size},
    js,
    text::Label,
};

use super::layered::{Kind, LEdge, LNode, LayeredOptions, Pin, layered, orient, rank};

const MAX_PORTS: usize = 40000;

#[derive(Clone, Debug, PartialEq)]
pub struct CNode {
    pub size: Size,
    pub parent: Option<usize>,
    pub is_group: bool,
    pub dir: Option<Direction>,
    pub pad_x: f64,
    pub pad_top: f64,
    pub pad_bottom: f64,
    pub min_w: f64,
    /// Declaration order; a group takes the smallest value among its members.
    pub seq: f64,
    /// How much of a side its edges may spread along, measured across the
    /// flow; 0 keeps them at its middle.
    pub span: f64,
    pub center: Point,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CEdge {
    pub src: usize,
    pub dst: usize,
    pub minlen: i32,
    pub label: Size,
    pub route: Vec<RoutePoint>,
    pub label_at: Point,
}

impl CEdge {
    /// Whether the end the edge leaves by (`out`) or arrives at is below
    /// group `item` rather than `item` itself.
    fn end_below(&self, item: usize, out: bool) -> bool {
        (if out { self.src } else { self.dst }) != item
    }

    /// The room `label` takes on an edge: its size and a margin, or none for
    /// an empty label.
    pub fn label_room(label: &Label) -> Size {
        Size::new(
            if label.width > 0.0 {
                label.width + 12.0
            } else {
                0.0
            },
            label.spaced_height(4.0),
        )
    }
}

impl CNode {
    pub fn new(size: Size, parent: Option<usize>) -> Self {
        CNode {
            size,
            parent,
            is_group: false,
            dir: None,
            pad_x: 0.0,
            pad_top: 0.0,
            pad_bottom: 0.0,
            min_w: 0.0,
            seq: 0.0,
            span: 0.0,
            center: Point::default(),
        }
    }

    /// The side of the layout box `p` lies on, within half a pixel.
    pub fn side_of(&self, p: Point) -> Option<Side> {
        let dx = p.x - self.center.x;
        let dy = p.y - self.center.y;
        if (dy.abs() - self.size.h / 2.0).abs() < 0.5 {
            return Some(if dy < 0.0 { Side::Top } else { Side::Bottom });
        }
        if (dx.abs() - self.size.w / 2.0).abs() < 0.5 {
            return Some(if dx < 0.0 { Side::Left } else { Side::Right });
        }
        None
    }

    /// The top left corner of the layout box.
    pub fn corner(&self) -> Point {
        Point::new(
            self.center.x - self.size.w / 2.0,
            self.center.y - self.size.h / 2.0,
        )
    }

    /// The point where the line from the center of this box to the center of
    /// `to` leaves this box. When that center is inside the box, the point is
    /// on the nearest side instead.
    pub fn border_toward(&self, to: &CNode) -> Point {
        let from = self;
        let dx = to.center.x - from.center.x;
        let dy = to.center.y - from.center.y;
        let hw = (from.size.w / 2.0).max(0.5);
        let hh = (from.size.h / 2.0).max(0.5);
        if dx.abs() < hw && dy.abs() < hh {
            let to_side = hw - dx.abs();
            let to_end = hh - dy.abs();
            return if to_side < to_end {
                Point::new(from.center.x + if dx < 0.0 { -hw } else { hw }, to.center.y)
            } else {
                Point::new(to.center.x, from.center.y + if dy < 0.0 { -hh } else { hh })
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
        Point::new(from.center.x + dx * scale, from.center.y + dy * scale)
    }

    /// A group laid out in `dir`, with its padding.
    pub fn group(dir: Option<Direction>, pad_x: f64, pad_top: f64, pad_bottom: f64) -> Self {
        CNode {
            is_group: true,
            dir,
            pad_x,
            pad_top,
            pad_bottom,
            ..CNode::new(Size::default(), None)
        }
    }

    /// Whether `p` lies on the border of the layout box, within half a
    /// pixel.
    pub fn on_border(&self, p: Point) -> bool {
        let dx = (p.x - self.center.x).abs();
        let dy = (p.y - self.center.y).abs();
        ((dy - self.size.h / 2.0).abs() < 0.5 && dx <= self.size.w / 2.0 + 0.5)
            || ((dx - self.size.w / 2.0).abs() < 0.5 && dy <= self.size.h / 2.0 + 0.5)
    }
}

struct PortEdge {
    e: usize,
    inner: usize,
    out: bool,
    side: Side,
    at: Point,
    route: Vec<RoutePoint>,
}

#[derive(Clone, Copy)]
struct LevelEdge {
    e: usize,
    a: usize,
    b: usize,
}

struct Level {
    /// The group laid out, or `None` for the root.
    id: Option<usize>,
    dir: Direction,
    items: Vec<usize>,
    edges: Vec<LevelEdge>,
    ports: Vec<PortEdge>,
    /// The ports of `ports` by `edge * 2`, plus 1 for the end an edge arrives
    /// at.
    port_of: HashMap<usize, usize>,
    routes: HashMap<usize, Vec<RoutePoint>>,
    origin: Point,
}

/// The levels of a layout, in the order they were made, and each by the
/// group it lays out.
struct Levels {
    list: Vec<Level>,
    of: HashMap<Option<usize>, usize>,
}

impl Levels {
    fn make(&mut self, id: Option<usize>, dir: Direction) -> usize {
        let at = self.list.len();
        self.list.push(Level {
            id,
            dir,
            items: Vec::new(),
            edges: Vec::new(),
            ports: Vec::new(),
            port_of: HashMap::new(),
            routes: HashMap::new(),
            origin: Point::default(),
        });
        self.of.insert(id, at);
        at
    }

    fn of(&self, id: usize) -> usize {
        self.of[&Some(id)]
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

    /// The flat graph of one level: its items, then a port node for every edge
    /// that crosses its border on the side the flow starts or ends at. `sized`
    /// gives the nodes their sizes and the edges their labels and offsets.
    fn flat_graph(
        &self,
        q: usize,
        nodes: &[CNode],
        edges: &[CEdge],
        local: &mut [usize],
        sized: bool,
    ) -> (Vec<LNode>, Vec<LEdge>, Vec<Option<usize>>) {
        let level = &self.list[q];
        let vertical = level.dir.is_vertical();
        let start = level.dir.start_side();
        let mut lnodes: Vec<LNode> = Vec::new();
        let mut ledges: Vec<LEdge> = Vec::new();
        for (i, &item) in level.items.iter().enumerate() {
            local[item] = i;
            let node = &nodes[item];
            let mut ln = if sized {
                LNode::new(
                    if vertical {
                        node.size
                    } else {
                        node.size.turned()
                    },
                    Kind::Node,
                )
            } else {
                LNode::new(Size::default(), Kind::Node)
            };
            ln.span = if node.is_group {
                node.span.min(ln.size.w - 2.0 * node.pad_x)
            } else {
                node.span
            };
            lnodes.push(ln);
        }
        let held = |item: usize, ei: usize, out: bool| {
            nodes[item].is_group && edges[ei].end_below(item, out)
        };
        let offset = |item: usize, ei: usize, out: bool| -> f64 {
            if !sized || !nodes[item].is_group || !edges[ei].end_below(item, out) {
                return 0.0;
            }
            let port = self.port(item, ei, out);
            if vertical {
                port.at.x - nodes[item].size.w / 2.0
            } else {
                port.at.y - nodes[item].size.h / 2.0
            }
        };
        for le in &level.edges {
            let e = &edges[le.e];
            let mut edge = LEdge::new(local[le.a], local[le.b], e.minlen, Size::default());
            if sized {
                edge.label = if vertical { e.label } else { e.label.turned() };
                edge.tail_dx = offset(le.a, le.e, true);
                edge.head_dx = offset(le.b, le.e, false);
                edge.tail_fixed = held(le.a, le.e, true);
                edge.head_fixed = held(le.b, le.e, false);
            }
            ledges.push(edge);
        }
        let mut dummies = vec![None; level.ports.len()];
        for (i, port) in level.ports.iter().enumerate() {
            if port.side == start || port.side == start.opposite() {
                let d = lnodes.len();
                let kind = if port.side == start {
                    Kind::StartPort
                } else {
                    Kind::EndPort
                };
                lnodes.push(LNode::new(Size::default(), kind));
                dummies[i] = Some(d);
                let mut edge = if port.out {
                    LEdge::new(local[port.inner], d, 1, Size::default())
                } else {
                    LEdge::new(d, local[port.inner], 1, Size::default())
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
                lnodes[local[port.inner]].pin =
                    Some(if matches!(port.side, Side::Top | Side::Left) {
                        Pin::First
                    } else {
                        Pin::Last
                    });
            }
        }
        (lnodes, ledges, dummies)
    }

    /// Places the self made from `from` on, and every level they hold, going
    /// down the tree breadth first.
    fn descend(
        &mut self,
        from: usize,
        nodes: &[CNode],
        children: &[Vec<usize>],
        depth: &mut [i32],
        placed: &mut [bool],
    ) {
        let mut q = from;
        while q < self.list.len() {
            let items = self.list[q].items.clone();
            let id = self.list[q].id;
            let dir = self.list[q].dir;
            for item in items {
                placed[item] = true;
                depth[item] = id.map_or(0, |id| depth[id] + 1);
                if nodes[item].is_group {
                    let child = self.make(Some(item), nodes[item].dir.unwrap_or(dir));
                    self.list[child].items = children[item].clone();
                }
            }
            q += 1;
        }
    }
}

pub fn shift_layout(nodes: &mut [CNode], edges: &mut [CEdge], by: Point) {
    for c in nodes.iter_mut() {
        c.center += by;
    }
    for e in edges.iter_mut() {
        for point in &mut e.route {
            point.at += by;
        }
        e.label_at += by;
    }
}

/// The group holding node `i`, which is below the root.
fn parent(nodes: &[CNode], i: usize) -> usize {
    nodes[i]
        .parent
        .expect("a node below the root is in a group")
}

fn append(route: &mut Vec<RoutePoint>, part: &[RoutePoint], origin: Point) {
    for point in part {
        let at = point.at + origin;
        if route.last().is_some_and(|last| last.at.near(at)) {
            continue;
        }
        route.push(RoutePoint::new(at, point.axis));
    }
}

/// `at` moved onto the border of a box of `size` on `side`.
fn onto(mut at: Point, side: Side, size: Size) -> Point {
    match side {
        Side::Top => at.y = 0.0,
        Side::Bottom => at.y = size.h,
        Side::Left => at.x = 0.0,
        Side::Right => at.x = size.w,
    }
    at
}

/// Lays out a graph whose nodes may be nested in groups. Each group is laid
/// out on its own, in its own direction, and then placed in its parent as a
/// single box. An edge that crosses a group's border passes through a port
/// on the side facing its other end.
pub fn compound_layout(
    nodes: &mut [CNode],
    edges: &mut [CEdge],
    root_dir: Direction,
    opt: &LayeredOptions,
) -> Size {
    let n = nodes.len();
    let mut levels = Levels {
        list: Vec::new(),
        of: HashMap::new(),
    };
    let root = levels.make(None, root_dir);

    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut root_items: Vec<usize> = Vec::new();
    for i in 0..n {
        match nodes[i].parent {
            Some(p) if p < n && p != i && nodes[p].is_group => children[p].push(i),
            _ => {
                nodes[i].parent = None;
                root_items.push(i);
            }
        }
    }
    levels.list[root].items = root_items;
    let mut depth = vec![0i32; n];
    let mut placed = vec![false; n];
    levels.descend(0, nodes, &children, &mut depth, &mut placed);
    // Groups that contain each other are unreachable from the root; lift them
    // out.
    for i in 0..n {
        if placed[i] {
            continue;
        }
        let former = parent(nodes, i);
        children[former].retain(|&c| c != i);
        nodes[i].parent = None;
        levels.list[root].items.push(i);
        placed[i] = true;
        depth[i] = 0;
        if nodes[i].is_group {
            let from = levels.list.len();
            let child = levels.make(Some(i), nodes[i].dir.unwrap_or(root_dir));
            levels.list[child].items = children[i].clone();
            levels.descend(from, nodes, &children, &mut depth, &mut placed);
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
        if seq != f64::INFINITY
            && let Some(id) = level.id
        {
            nodes[id].seq = seq;
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
                    at: Point::default(),
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
            if nodes[inner].is_group && edges[e].end_below(inner, out) {
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
        let (mut lnodes, mut ledges, _) = levels.flat_graph(q, nodes, edges, &mut local, false);
        orient(&lnodes, &mut ledges);
        rank(&mut lnodes, &ledges, 1);
        let start = levels.list[q].dir.start_side();
        for (i, le) in level_edges.iter().enumerate() {
            let e = &edges[le.e];
            let a_above = !ledges[i].reversed;
            if nodes[le.a].is_group && e.src != le.a {
                levels.port_mut(le.a, le.e, true).side =
                    if a_above { start.opposite() } else { start };
            }
            if nodes[le.b].is_group && e.dst != le.b {
                levels.port_mut(le.b, le.e, false).side =
                    if a_above { start } else { start.opposite() };
            }
        }
    }

    // Bottom-up: lay out each level now that the sizes of its groups are
    // known.
    let mut total = Size::default();
    for q in (0..levels.list.len()).rev() {
        let group = levels.list[q].id;
        let dir = levels.list[q].dir;
        let vertical = dir.is_vertical();
        let flip = dir.is_reversed();
        let (mut lnodes, mut ledges, dummies) =
            levels.flat_graph(q, nodes, edges, &mut local, true);
        let size = layered(&mut lnodes, &mut ledges, opt);
        let content = if vertical { size } else { size.turned() };
        let pad_x = group.map_or(0.0, |g| nodes[g].pad_x);
        let pad_top = group.map_or(0.0, |g| nodes[g].pad_top);
        let boxed = Size::new(
            (content.w + 2.0 * pad_x).max(group.map_or(0.0, |g| nodes[g].min_w)),
            content.h + pad_top + group.map_or(0.0, |g| nodes[g].pad_bottom),
        );
        let off_x = (boxed.w - content.w) / 2.0;
        let off_y = pad_top;
        let to_box = |p: Point| {
            Point::new(
                off_x
                    + if vertical {
                        p.x
                    } else if flip {
                        size.h - p.y
                    } else {
                        p.y
                    },
                off_y
                    + if vertical {
                        if flip { size.h - p.y } else { p.y }
                    } else {
                        p.x
                    },
            )
        };
        let axis = dir.axis();
        let convert = |points: &[Point]| -> Vec<RoutePoint> {
            points
                .iter()
                .map(|&p| RoutePoint::new(to_box(p), axis))
                .collect()
        };

        for (i, &item) in levels.list[q].items.iter().enumerate() {
            nodes[item].center = to_box(lnodes[i].center);
        }
        let level_edges = levels.list[q].edges.clone();
        for (i, le) in level_edges.iter().enumerate() {
            let edge = &ledges[i];
            levels.list[q].routes.insert(le.e, convert(&edge.points));
            let e = &mut edges[le.e];
            if e.label.w > 0.0 {
                e.label_at = to_box(edge.label_at);
            }
        }
        let mut k = level_edges.len();
        for (i, &dummy) in dummies.iter().enumerate() {
            let (inner, e, out, side) = {
                let port = &levels.list[q].ports[i];
                (port.inner, port.e, port.out, port.side)
            };
            if dummy.is_some() {
                let mut route = convert(&ledges[k].points);
                k += 1;
                let end = if out {
                    route[route.len() - 1]
                } else {
                    route[0]
                };
                let border = RoutePoint::new(onto(end.at, side, boxed), axis);
                if out {
                    route.push(border);
                } else {
                    route.insert(0, border);
                }
                let port = &mut levels.list[q].ports[i];
                port.at = border.at;
                port.route = route;
                continue;
            }
            let item = &nodes[inner];
            let mut inside = item.center;
            if item.is_group && edges[e].end_below(inner, out) {
                let below = levels.port(inner, e, out);
                inside.x += below.at.x - item.size.w / 2.0;
                inside.y += below.at.y - item.size.h / 2.0;
            } else {
                match side {
                    Side::Top => inside.y -= item.size.h / 2.0,
                    Side::Bottom => inside.y += item.size.h / 2.0,
                    Side::Left => inside.x -= item.size.w / 2.0,
                    Side::Right => inside.x += item.size.w / 2.0,
                }
            }
            let border = onto(inside, side, boxed);
            let a = if side.is_horizontal() {
                Axis::Y
            } else {
                Axis::X
            };
            let port = &mut levels.list[q].ports[i];
            port.at = border;
            port.route = if out {
                vec![RoutePoint::new(inside, a), RoutePoint::new(border, a)]
            } else {
                vec![RoutePoint::new(border, a), RoutePoint::new(inside, a)]
            };
        }

        if let Some(g) = group {
            nodes[g].size = boxed;
        } else {
            total = boxed;
        }
    }

    // Top-down again: turn positions inside each box into diagram
    // coordinates.
    for q in 0..levels.list.len() {
        let origin = levels.list[q].origin;
        for i in 0..levels.list[q].items.len() {
            let item = levels.list[q].items[i];
            let node = &mut nodes[item];
            node.center += origin;
            if node.is_group {
                let corner = node.corner();
                let child = levels.of(item);
                levels.list[child].origin = corner;
            }
        }
    }

    for level in &levels.list {
        for le in &level.edges {
            let e = &edges[le.e];
            let mut route: Vec<RoutePoint> = Vec::new();
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
                    lv.origin,
                );
            }
            append(&mut route, &level.routes[&le.e], level.origin);
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
                    lv.origin,
                );
            }
            let e = &mut edges[le.e];
            e.route = route;
            if e.label.w > 0.0 {
                e.label_at += level.origin;
            }
        }
    }

    for ei in stray {
        let a = nodes[edges[ei].src].center;
        let b = nodes[edges[ei].dst].center;
        let e = &mut edges[ei];
        e.route = vec![RoutePoint::new(a, Axis::Y), RoutePoint::new(b, Axis::Y)];
        e.label_at = a.midpoint(b);
    }
    for ei in loops {
        let node = &nodes[edges[ei].src];
        let (center, w) = (node.center, node.size.w);
        let e = &mut edges[ei];
        e.label_at = Point::new(center.x + w / 2.0 + 28.0 + e.label.w / 2.0, center.y);
    }

    total
}

/// The direction of the nearest of the first 10000 groups above node `i`
/// that sets one, or `root`.
pub fn level_dir(nodes: &[CNode], i: usize, root: Direction) -> Direction {
    std::iter::successors(nodes[i].parent, |&at| nodes[at].parent)
        .take(10000)
        .find_map(|at| nodes[at].dir)
        .unwrap_or(root)
}

/// Spreads the route ends that meet one side of a node along that side, at
/// most `gap` apart and `margin` clear of its corners, in the order of where
/// their edges head. `drawn` gives the size node `i` is drawn at, centered
/// in its layout box. Self-loops and routes of fewer than two points are
/// left alone.
pub fn spread_ends(
    nodes: &[CNode],
    edges: &mut [CEdge],
    drawn: impl Fn(usize) -> Size,
    gap: f64,
    margin: f64,
) {
    struct End {
        edge: usize,
        at: usize,
        toward: f64,
    }
    let mut sides: indexmap::IndexMap<(usize, Side), Vec<End>> = indexmap::IndexMap::new();
    for (e, edge) in edges.iter().enumerate() {
        let route = &edge.route;
        if edge.src == edge.dst || route.len() < 2 {
            continue;
        }
        for at in [0, route.len() - 1] {
            let node = if at == 0 { edge.src } else { edge.dst };
            let next = route[if at == 0 { 1 } else { at - 1 }].at;
            let Some(side) = nodes[node].side_of(route[at].at) else {
                continue;
            };
            let end = End {
                edge: e,
                at,
                toward: if side.is_horizontal() { next.x } else { next.y },
            };
            sides.entry((node, side)).or_default().push(end);
        }
    }
    for (&(node, side), ends) in sides.iter_mut() {
        if ends.len() < 2 {
            continue;
        }
        let axis = if side.is_horizontal() {
            Axis::X
        } else {
            Axis::Y
        };
        let length = drawn(node).along(axis);
        let step = gap
            .min((length - 2.0 * margin) / (ends.len() - 1) as f64)
            .max(0.0);
        let center = nodes[node].center.along(axis);
        ends.sort_by(|a, b| js::compare(a.toward - b.toward));
        let count = ends.len() as f64;
        for (j, end) in ends.iter().enumerate() {
            *edges[end.edge].route[end.at].at.along_mut(axis) =
                center + (j as f64 - (count - 1.0) / 2.0) * step;
        }
    }
}

/// The groups around node `i`, innermost first. The walk ends after
/// `nodes.len()` steps, so a cycle of parents does not hang it.
pub fn ancestors(nodes: &[CNode], i: usize) -> impl Iterator<Item = usize> + '_ {
    std::iter::successors(nodes[i].parent, |&at| nodes[at].parent).take(nodes.len())
}
