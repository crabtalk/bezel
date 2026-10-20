//! Grid placement. Pele's `diagrams/architecture/layout.ts`.
//!
//! Every service and junction takes a cell, an edge `a:R -- L:b` puts b in
//! the cell to the right of a, and a group is a solid rectangle of cells, so
//! a group never encloses a node that is not in it. Each container (the
//! diagram, then every group) is laid out on its own, innermost first, and a
//! finished group is placed in its parent as one block.

use std::collections::HashMap;

use super::{
    db::{ArchEdge, ArchGroup, ArchLayoutHint, ArchNode},
    parser::Alignment,
};
use crate::geom::Side;

/// Blocks up to this many cells are found through the cell map; larger ones
/// are compared one by one.
const SMALL: i64 = 64;
const OFFSET: i64 = 1 << 25;
const SPAN: i64 = 1 << 26;

/// Where every node and group landed, in cells.
pub struct Grid {
    /// In the order of the nodes passed in.
    pub nodes: Vec<GridNode>,
    /// In the order of the groups passed in.
    pub groups: Vec<GridGroup>,
}

pub struct GridNode {
    pub col: i64,
    pub row: i64,
    /// The index of its group.
    pub parent: Option<usize>,
}

/// A group's rectangle of cells. A group with nothing in it takes one cell.
pub struct GridGroup {
    pub col: i64,
    pub row: i64,
    pub cols: i64,
    pub rows: i64,
    /// The index of the group it is in.
    pub parent: Option<usize>,
    pub empty: bool,
}

/// One cell's step out through `side`.
fn step(side: Side) -> (i64, i64) {
    let out = side.outward();
    (out.x as i64, out.y as i64)
}

/// A rectangle of cells.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Block {
    x: i64,
    y: i64,
    w: i64,
    h: i64,
}

impl Block {
    fn overlaps(self, other: Block) -> bool {
        self.x < other.x + other.w
            && other.x < self.x + self.w
            && self.y < other.y + other.h
            && other.y < self.y + self.h
    }

    /// Moved `k` steps of (`sx`, `sy`).
    fn moved(self, k: i64, sx: i64, sy: i64) -> Block {
        Block {
            x: self.x + k * sx,
            y: self.y + k * sy,
            ..self
        }
    }
}

/// One end of a link: the node, the side of it, and the child of the link's
/// container that holds the node.
#[derive(Clone, Copy)]
struct End {
    node: usize,
    side: Side,
    item: usize,
}

#[derive(Clone, Copy)]
struct Link {
    a: End,
    b: End,
}

struct Placer {
    n: usize,
    parent: Vec<usize>,
    edge_links: Vec<Option<Vec<usize>>>,
    hint_links: Vec<Option<Vec<usize>>>,
    links: Vec<Link>,
    /// Each item's place in its container and its size.
    blocks: Vec<Block>,
    placed: Vec<bool>,
    order: Vec<usize>,
    cells: HashMap<i64, usize>,
    large: Vec<usize>,
    reach: HashMap<(Block, i64, i64), i64>,
    first: usize,
}

impl Placer {
    /// Where `node` sits within `item`, the item that holds it.
    fn anchor(&self, node: usize, item: usize) -> (i64, i64) {
        let (mut ax, mut ay) = (0, 0);
        let mut cur = node;
        while cur != item {
            ax += self.blocks[cur].x;
            ay += self.blocks[cur].y;
            cur = self.n + self.parent[cur];
        }
        (ax, ay)
    }

    fn put(&mut self, item: usize, px: i64, py: i64) {
        let block = &mut self.blocks[item];
        block.x = px;
        block.y = py;
        let block = *block;
        self.placed[item] = true;
        self.order.push(item);
        if block.w * block.h > SMALL {
            self.large.push(item);
            return;
        }
        for cy in py..py + block.h {
            for cx in px..px + block.w {
                self.cells.insert((cx + OFFSET) * SPAN + cy + OFFSET, item);
            }
        }
    }

    fn collide(&self, rect: Block) -> Option<usize> {
        if rect.w * rect.h > SMALL {
            return self.order[self.first..]
                .iter()
                .copied()
                .find(|&item| self.blocks[item].overlaps(rect));
        }
        for cy in rect.y..rect.y + rect.h {
            for cx in rect.x..rect.x + rect.w {
                if let Some(&hit) = self.cells.get(&((cx + OFFSET) * SPAN + cy + OFFSET)) {
                    return Some(hit);
                }
            }
        }
        self.large
            .iter()
            .copied()
            .find(|&item| self.blocks[item].overlaps(rect))
    }

    /// How far a block must slide along (`sx`, `sy`) to be clear. Each step
    /// jumps past what it hit, and the result is kept, because a later block
    /// starting from the same place cannot stop sooner.
    fn slide(&mut self, rect: Block, sx: i64, sy: i64) -> i64 {
        let key = (rect, sx, sy);
        let mut k = self.reach.get(&key).copied().unwrap_or(0);
        while let Some(hit) = self.collide(rect.moved(k, sx, sy)) {
            let hit = self.blocks[hit];
            k = if sx > 0 {
                hit.x + hit.w - rect.x
            } else if sx < 0 {
                rect.x + rect.w - hit.x
            } else if sy > 0 {
                hit.y + hit.h - rect.y
            } else {
                rect.y + rect.h - hit.y
            };
        }
        self.reach.insert(key, k);
        k
    }

    fn follow(&mut self, from: usize, k: usize) {
        let link = self.links[k];
        let (near, far) = if link.a.item == from {
            (link.a, link.b)
        } else {
            (link.b, link.a)
        };
        let to = far.item;
        if self.placed[to] {
            return;
        }
        let (sp, sq) = (near.side, far.side);
        let (pax, pay) = self.anchor(near.node, from);
        let (ax, ay) = self.anchor(far.node, to);
        let (spx, spy) = step(sp);
        let (sqx, sqy) = step(sq);
        let (mut dx, mut dy) = (spx, spy);
        let mut shift_x = 0;
        let mut shift_y = 0;
        let facing = spx + sqx == 0 && spy + sqy == 0;
        if sp == sq {
            // Like sides: the edge loops around, so the nodes stack across
            // the direction it leaves in.
            dx = if spx == 0 { 1 } else { 0 };
            dy = 1 - dx;
        } else if !facing {
            // A bend: one step along the side the edge leaves, one step back
            // from the side it enters.
            shift_x = -sqx;
            shift_y = -sqy;
        }
        let (f, t) = (self.blocks[from], self.blocks[to]);
        let mut bx;
        let mut by;
        if dx != 0 {
            bx = if dx > 0 { f.x + f.w } else { f.x - t.w };
            by = f.y + pay + shift_y - ay;
        } else {
            by = if dy > 0 { f.y + f.h } else { f.y - t.h };
            bx = f.x + pax + shift_x - ax;
        }
        if facing {
            // When several placed nodes reach this one through the same pair
            // of sides, line it up with the middle one, not with whichever
            // happened to be visited first.
            if let Some(list) = self.edge_links[to].clone()
                && list.len() > 1
            {
                let mut wanted: Vec<i64> = Vec::new();
                for j in list {
                    let link = self.links[j];
                    let (there, other) = if link.b.item == to {
                        (link.b, link.a)
                    } else {
                        (link.a, link.b)
                    };
                    if !self.placed[other.item] || other.side != sp || there.side != sq {
                        continue;
                    }
                    let (oax, oay) = self.anchor(other.node, other.item);
                    let o = self.blocks[other.item];
                    let at = if dx != 0 { o.y + oay } else { o.x + oax };
                    let (tax, tay) = self.anchor(there.node, to);
                    wanted.push(at - if dx != 0 { tay } else { tax });
                }
                if wanted.len() > 1 {
                    wanted.sort();
                    let middle = wanted[(wanted.len() - 1) >> 1];
                    if dx != 0 {
                        by = middle;
                    } else {
                        bx = middle;
                    }
                }
            }
            // Taken: move sideways, to whichever side is nearer, so the edge
            // can still leave straight.
            let px = if dx == 0 { 1 } else { 0 };
            let py = 1 - px;
            let rect = Block { x: bx, y: by, ..t };
            let forth = self.slide(rect, px, py);
            let back = if forth == 0 {
                0
            } else {
                self.slide(rect, -px, -py)
            };
            let near = if forth <= back { forth } else { -back };
            bx += px * near;
            by += py * near;
        } else {
            let far = self.slide(Block { x: bx, y: by, ..t }, dx, dy);
            bx += dx * far;
            by += dy * far;
        }
        self.put(to, bx, by);
    }

    fn layout(&mut self, kids: &[usize], g_count: usize, c: usize) {
        if kids.is_empty() {
            return;
        }
        self.order.clear();
        let mut starts: Vec<usize> = Vec::new();
        for &start in kids {
            if self.placed[start] {
                continue;
            }
            self.first = self.order.len();
            starts.push(self.first);
            self.cells.clear();
            self.large.clear();
            self.reach.clear();
            self.put(start, 0, 0);
            // Declared alignments are followed as soon as a node is placed,
            // ahead of any edge.
            let mut hint_at = self.first;
            let mut edge_at = self.first;
            let mut link_at = 0;
            loop {
                if hint_at < self.order.len() {
                    let from = self.order[hint_at];
                    hint_at += 1;
                    if let Some(list) = self.hint_links[from].clone() {
                        for k in list {
                            self.follow(from, k);
                        }
                    }
                    continue;
                }
                if edge_at >= self.order.len() {
                    break;
                }
                let from = self.order[edge_at];
                let next = self.edge_links[from]
                    .as_ref()
                    .and_then(|list| list.get(link_at).copied());
                let Some(k) = next else {
                    edge_at += 1;
                    link_at = 0;
                    continue;
                };
                link_at += 1;
                self.follow(from, k);
            }
        }
        starts.push(self.order.len());

        // Unconnected parts go side by side, wrapping into rows that keep the
        // whole roughly square.
        let count = starts.len() - 1;
        let mut parts: Vec<Block> = Vec::with_capacity(count);
        let mut area = 0i64;
        let mut widest = 0i64;
        for p in 0..count {
            let (mut x0, mut y0, mut x1, mut y1) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
            for &item in &self.order[starts[p]..starts[p + 1]] {
                let block = self.blocks[item];
                x0 = x0.min(block.x);
                y0 = y0.min(block.y);
                x1 = x1.max(block.x + block.w);
                y1 = y1.max(block.y + block.h);
            }
            let part = Block {
                x: x0,
                y: y0,
                w: x1 - x0,
                h: y1 - y0,
            };
            area += part.w * part.h;
            widest = widest.max(part.w);
            parts.push(part);
        }
        let limit = (widest as f64).max(((area * 2) as f64).sqrt().ceil());
        let mut cx = 0i64;
        let mut cy = 0i64;
        let mut shelf = 0i64;
        let mut width = 0i64;
        for (p, part) in parts.iter().enumerate() {
            if cx > 0 && (cx + part.w) as f64 > limit {
                cy += shelf;
                cx = 0;
                shelf = 0;
            }
            for i in starts[p]..starts[p + 1] {
                let block = &mut self.blocks[self.order[i]];
                block.x += cx - part.x;
                block.y += cy - part.y;
            }
            cx += part.w;
            shelf = shelf.max(part.h);
            width = width.max(cx);
        }
        if c < g_count {
            let block = &mut self.blocks[self.n + c];
            block.w = width;
            block.h = cy + shelf;
        }
    }

    /// A link asks for one node to sit beside another. It belongs to the
    /// innermost container that holds both ends, and there it relates the two
    /// children of that container the ends are in.
    fn add_link(&mut self, depth: &[i64], a: usize, b: usize, sa: Side, sb: Side, hint: bool) {
        if a == b {
            return;
        }
        let n = self.n;
        let mut ia = a;
        let mut ib = b;
        let mut ca = self.parent[a];
        let mut cb = self.parent[b];
        while ca != cb {
            if depth[ca] >= depth[cb] {
                ia = n + ca;
                ca = self.parent[ia];
            } else {
                ib = n + cb;
                cb = self.parent[ib];
            }
        }
        let k = self.links.len();
        self.links.push(Link {
            a: End {
                node: a,
                side: sa,
                item: ia,
            },
            b: End {
                node: b,
                side: sb,
                item: ib,
            },
        });
        let lists = if hint {
            &mut self.hint_links
        } else {
            &mut self.edge_links
        };
        lists[ia].get_or_insert_with(Vec::new).push(k);
        lists[ib].get_or_insert_with(Vec::new).push(k);
    }
}

pub fn place_on_grid(
    nodes: &[&ArchNode],
    groups: &[&ArchGroup],
    edges: &[ArchEdge],
    hints: &[ArchLayoutHint],
) -> Grid {
    let n = nodes.len();
    let g_count = groups.len();
    let root = g_count;
    let mut node_index: HashMap<&str, usize> = HashMap::new();
    let mut group_index: HashMap<&str, usize> = HashMap::new();
    // Items are nodes (0..N-1) then groups (N..N+G-1). Containers are groups
    // (0..G-1) then the root.
    let mut parent = vec![root; n + g_count];
    let mut depth = vec![0i64; g_count + 1];
    let mut kids: Vec<Vec<usize>> = vec![Vec::new(); g_count + 1];
    for (g, group) in groups.iter().enumerate() {
        group_index.insert(&group.id, g);
        let p = group
            .parent
            .as_deref()
            .and_then(|id| group_index.get(id).copied());
        match p {
            Some(p) if p != g => {
                parent[n + g] = p;
                depth[g] = depth[p] + 1;
            }
            _ => depth[g] = 1,
        }
    }
    for (i, node) in nodes.iter().enumerate() {
        node_index.insert(&node.id, i);
        if let Some(p) = node
            .parent
            .as_deref()
            .and_then(|id| group_index.get(id).copied())
        {
            parent[i] = p;
        }
        kids[parent[i]].push(i);
    }
    for g in 0..g_count {
        kids[parent[n + g]].push(n + g);
    }

    let mut placer = Placer {
        n,
        parent,
        edge_links: vec![None; n + g_count],
        hint_links: vec![None; n + g_count],
        links: Vec::new(),
        blocks: vec![
            Block {
                x: 0,
                y: 0,
                w: 1,
                h: 1
            };
            n + g_count
        ],
        placed: vec![false; n + g_count],
        order: Vec::new(),
        cells: HashMap::new(),
        large: Vec::new(),
        reach: HashMap::new(),
        first: 0,
    };
    for hint in hints {
        let row = hint.direction == Alignment::Row;
        for i in 1..hint.members.len() {
            let a = node_index.get(hint.members[i - 1].as_str()).copied();
            let b = node_index.get(hint.members[i].as_str()).copied();
            if let (Some(a), Some(b)) = (a, b) {
                let (sa, sb) = if row {
                    (Side::Right, Side::Left)
                } else {
                    (Side::Bottom, Side::Top)
                };
                placer.add_link(&depth, a, b, sa, sb, true);
            }
        }
    }
    // An edge between two like sides says least about where its ends go, so
    // those come last.
    for like_sides in [false, true] {
        for edge in edges {
            if (edge.lhs_dir == edge.rhs_dir) != like_sides {
                continue;
            }
            let a = node_index.get(edge.lhs_id.as_str()).copied();
            let b = node_index.get(edge.rhs_id.as_str()).copied();
            if let (Some(a), Some(b)) = (a, b) {
                placer.add_link(&depth, a, b, edge.lhs_dir, edge.rhs_dir, false);
            }
        }
    }

    for c in (0..g_count).rev() {
        placer.layout(&kids[c], g_count, c);
    }
    placer.layout(&kids[root], g_count, root);

    let parent = &placer.parent;
    let inside = |p: usize| (p != root).then_some(p);
    let mut grid_groups: Vec<GridGroup> = Vec::with_capacity(g_count);
    for g in 0..g_count {
        let parent = inside(parent[n + g]);
        let block = placer.blocks[n + g];
        let (col, row) = match parent {
            Some(p) => (block.x + grid_groups[p].col, block.y + grid_groups[p].row),
            None => (block.x, block.y),
        };
        grid_groups.push(GridGroup {
            col,
            row,
            cols: block.w,
            rows: block.h,
            parent,
            empty: kids[g].is_empty(),
        });
    }
    let grid_nodes = (0..n)
        .map(|i| {
            let parent = inside(parent[i]);
            let block = placer.blocks[i];
            let (col, row) = match parent {
                Some(p) => (block.x + grid_groups[p].col, block.y + grid_groups[p].row),
                None => (block.x, block.y),
            };
            GridNode { col, row, parent }
        })
        .collect();
    Grid {
        nodes: grid_nodes,
        groups: grid_groups,
    }
}
