//! The compound and lane layouts: Pele's tests of them, case for case.

use std::collections::HashSet;

use markdown::mermaid::{
    layout::{
        compound::{CEdge, CNode, Dir, cnode, compound_layout},
        lanes::lane_layout,
        layered::LayeredOptions,
    },
    svg::shapes::end_side,
};

const OPTIONS: LayeredOptions = LayeredOptions {
    node_sep: 40.0,
    edge_sep: 16.0,
    rank_sep: 48.0,
    port_sep: 20.0,
    tracks: false,
    straight: false,
    apart: false,
    head_room: None,
};
const DIRS: [Dir; 4] = [Dir::TB, Dir::BT, Dir::LR, Dir::RL];

/// Pele's test generator: a linear congruential sequence in [0, 1).
struct Random(u32);

impl Random {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        self.0 as f64 / 4294967296.0
    }

    /// `Math.floor(rnd() * n)`.
    fn int(&mut self, n: usize) -> usize {
        (self.next() * n as f64).floor() as usize
    }
}

/// `toBeCloseTo(expected, digits)`.
fn close(actual: f64, expected: f64, digits: i32) -> bool {
    (actual - expected).abs() < 10f64.powi(-digits) / 2.0
}

/// `Math.round`.
fn round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

fn edge(src: usize, dst: usize, label_w: f64, label_h: f64) -> CEdge {
    CEdge {
        src,
        dst,
        minlen: 1,
        label_w,
        label_h,
        ..CEdge::default()
    }
}

fn link(src: usize, dst: usize) -> CEdge {
    edge(src, dst, 0.0, 0.0)
}

fn on_border(node: &CNode, x: f64, y: f64) -> bool {
    let dx = (x - node.x).abs();
    let dy = (y - node.y).abs();
    let inside = dx <= node.w / 2.0 + 0.5 && dy <= node.h / 2.0 + 0.5;
    let edge_x = (dx - node.w / 2.0).abs() <= 0.5;
    let edge_y = (dy - node.h / 2.0).abs() <= 0.5;
    inside && (edge_x || edge_y)
}

fn is_ancestor(nodes: &[CNode], a: usize, b: usize) -> bool {
    let mut p = nodes[b].parent;
    while p >= 0 {
        if p as usize == a {
            return true;
        }
        p = nodes[p as usize].parent;
    }
    false
}

fn seq(nodes: &mut [CNode]) {
    for (i, node) in nodes.iter_mut().enumerate() {
        node.seq = i as f64;
    }
}

fn spans(nodes: &mut [CNode], span: f64) {
    for node in nodes.iter_mut() {
        node.span = span;
    }
}

fn boxed(c: &CNode) -> [f64; 4] {
    [
        c.x - c.w / 2.0,
        c.y - c.h / 2.0,
        c.x + c.w / 2.0,
        c.y + c.h / 2.0,
    ]
}

/// Every other graph lets edges spread along the sides of its nodes.
fn compound_graph(seed: u32) -> (Vec<CNode>, Vec<CEdge>, Dir) {
    let mut draws = Random(seed);
    let mut nodes: Vec<CNode> = Vec::new();
    let mut groups: Vec<usize> = Vec::new();
    let group_count = draws.int(5);
    for g in 0..group_count {
        let parent = if !groups.is_empty() && draws.next() < 0.4 {
            groups[draws.int(groups.len())] as i32
        } else {
            -1
        };
        let mut node = cnode(0.0, 0.0, parent);
        node.is_group = true;
        node.pad_x = 20.0;
        node.pad_top = 40.0;
        node.pad_bottom = 20.0;
        node.min_w = 60.0;
        node.dir = if draws.next() < 0.3 {
            Some(DIRS[draws.int(4)])
        } else {
            None
        };
        node.seq = (1000 + g) as f64;
        if seed.is_multiple_of(2) {
            node.span = f64::INFINITY;
        }
        groups.push(nodes.len());
        nodes.push(node);
    }
    let leaves = 3 + draws.int(30);
    for i in 0..leaves {
        let parent = if !groups.is_empty() && draws.next() < 0.5 {
            groups[draws.int(groups.len())] as i32
        } else {
            -1
        };
        let w = (30 + draws.int(120)) as f64;
        let h = (30 + draws.int(50)) as f64;
        let mut node = cnode(w, h, parent);
        node.seq = i as f64;
        if seed.is_multiple_of(2) {
            node.span = node.w.min(node.h) - 24.0;
        }
        nodes.push(node);
    }
    let mut edges: Vec<CEdge> = Vec::new();
    let count = draws.int(leaves * 2);
    for _ in 0..count {
        let labelled = draws.next() < 0.3;
        let src = draws.int(nodes.len());
        let dst = draws.int(nodes.len());
        let label_w = if labelled {
            (20 + draws.int(60)) as f64
        } else {
            0.0
        };
        edges.push(edge(src, dst, label_w, if labelled { 24.0 } else { 0.0 }));
    }
    let dir = DIRS[draws.int(4)];
    (nodes, edges, dir)
}

#[test]
fn compound_keeps_random_graphs_well_formed() {
    for seed in 1..=300 {
        let (mut nodes, mut edges, dir) = compound_graph(seed);
        let size = compound_layout(&mut nodes, &mut edges, dir, &OPTIONS);
        let at = format!("seed {seed}");

        assert!(size.width.is_finite() && size.height.is_finite(), "{at}");
        for (i, node) in nodes.iter().enumerate() {
            assert!(node.x.is_finite() && node.y.is_finite(), "{at} node {i}");
            assert!(node.x - node.w / 2.0 >= -0.5, "{at} node {i} left");
            assert!(node.y - node.h / 2.0 >= -0.5, "{at} node {i} top");
            assert!(
                node.x + node.w / 2.0 <= size.width + 0.5,
                "{at} node {i} right"
            );
            assert!(
                node.y + node.h / 2.0 <= size.height + 0.5,
                "{at} node {i} bottom"
            );
            if node.parent >= 0 {
                let parent = &nodes[node.parent as usize];
                assert!(
                    (node.x - parent.x).abs() + node.w / 2.0 <= parent.w / 2.0 + 0.5,
                    "{at} node {i} in parent"
                );
                assert!(
                    (node.y - parent.y).abs() + node.h / 2.0 <= parent.h / 2.0 + 0.5,
                    "{at} node {i} in parent"
                );
            }
        }

        for a in 0..nodes.len() {
            for b in a + 1..nodes.len() {
                if nodes[a].parent != nodes[b].parent {
                    continue;
                }
                let overlap_x = (nodes[a].w + nodes[b].w) / 2.0 - (nodes[a].x - nodes[b].x).abs();
                let overlap_y = (nodes[a].h + nodes[b].h) / 2.0 - (nodes[a].y - nodes[b].y).abs();
                assert!(
                    !(overlap_x > 0.5 && overlap_y > 0.5),
                    "{at} nodes {a} and {b} overlap"
                );
            }
        }

        for (i, e) in edges.iter().enumerate() {
            if e.src == e.dst {
                continue;
            }
            assert_eq!(e.route.len() % 3, 0, "{at} edge {i}");
            assert!(e.route.len() >= 6, "{at} edge {i} has a route");
            for value in &e.route {
                assert!(value.is_finite(), "{at} edge {i}");
            }
            if is_ancestor(&nodes, e.src, e.dst) || is_ancestor(&nodes, e.dst, e.src) {
                continue;
            }
            let last = e.route.len() - 3;
            assert!(
                on_border(&nodes[e.src], e.route[0], e.route[1]),
                "{at} edge {i} starts on its source"
            );
            assert!(
                on_border(&nodes[e.dst], e.route[last], e.route[last + 1]),
                "{at} edge {i} ends on its target"
            );
        }
    }
}

#[test]
fn compound_is_deterministic() {
    for seed in 1..=40 {
        let (mut first_nodes, mut first_edges, first_dir) = compound_graph(seed);
        let (mut second_nodes, mut second_edges, second_dir) = compound_graph(seed);
        compound_layout(&mut first_nodes, &mut first_edges, first_dir, &OPTIONS);
        compound_layout(&mut second_nodes, &mut second_edges, second_dir, &OPTIONS);
        let shape = |nodes: &[CNode]| {
            nodes
                .iter()
                .map(|n| [n.x, n.y, n.w, n.h])
                .collect::<Vec<_>>()
        };
        assert_eq!(shape(&second_nodes), shape(&first_nodes));
        let routes = |edges: &[CEdge]| edges.iter().map(|e| e.route.clone()).collect::<Vec<_>>();
        assert_eq!(routes(&second_edges), routes(&first_edges));
    }
}

#[test]
fn compound_runs_ranks_along_the_requested_direction() {
    for dir in DIRS {
        let mut nodes = vec![
            cnode(60.0, 40.0, -1),
            cnode(60.0, 40.0, -1),
            cnode(60.0, 40.0, -1),
        ];
        seq(&mut nodes);
        compound_layout(&mut nodes, &mut [link(0, 1), link(1, 2)], dir, &OPTIONS);
        let vertical = dir == Dir::TB || dir == Dir::BT;
        let axis = |n: &CNode| if vertical { n.y } else { n.x };
        let sign = if dir == Dir::TB || dir == Dir::LR {
            1.0
        } else {
            -1.0
        };
        assert!(sign * (axis(&nodes[1]) - axis(&nodes[0])) > 40.0, "{dir:?}");
        assert!(sign * (axis(&nodes[2]) - axis(&nodes[1])) > 40.0, "{dir:?}");
    }
}

#[test]
fn compound_keeps_a_long_edge_straight_through_the_ranks_it_skips() {
    let mut nodes = vec![
        cnode(60.0, 40.0, -1),
        cnode(60.0, 40.0, -1),
        cnode(60.0, 40.0, -1),
    ];
    seq(&mut nodes);
    let mut edges = vec![link(0, 1), link(1, 2), link(0, 2)];
    compound_layout(&mut nodes, &mut edges, Dir::TB, &OPTIONS);
    let route = &edges[2].route;
    let mut xs = HashSet::new();
    let mut i = 3;
    while i < route.len() - 3 {
        xs.insert(round(route[i]));
        i += 3;
    }
    assert_eq!(xs.len(), 1);
}

#[test]
fn compound_runs_an_edge_that_skips_ranks_in_line_with_one_of_its_ends_so_that_it_turns_once_at_the_other()
 {
    // A chain of eight, with edges that skip ranks down it and back up.
    let mut nodes: Vec<CNode> = (0..8).map(|_| cnode(80.0, 40.0, -1)).collect();
    seq(&mut nodes);
    spans(&mut nodes, 56.0);
    let mut edges: Vec<CEdge> = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 4),
        (4, 5),
        (5, 6),
        (6, 7),
        (0, 3),
        (0, 7),
        (1, 5),
        (2, 6),
        (6, 1),
        (7, 0),
    ]
    .into_iter()
    .map(|(a, b)| link(a, b))
    .collect();
    compound_layout(&mut nodes, &mut edges, Dir::TB, &OPTIONS);
    // How many times the route changes its place across the flow.
    let turns = |route: &[f64]| {
        (3..route.len())
            .step_by(3)
            .filter(|&i| (route[i] - route[i - 3]).abs() > 0.01)
            .count()
    };
    for e in &edges[7..] {
        assert_eq!(turns(&e.route), 1, "{} to {}", e.src, e.dst);
    }
    // The edge from the second node down and the one back up to it both meet
    // it in line.
    let down = &edges[9].route;
    let up = &edges[11].route;
    assert!(close(down[3], down[0], 5));
    assert!(close(up[up.len() - 6], up[up.len() - 3], 5));
}

#[test]
fn compound_keeps_the_edges_on_one_side_of_a_node_at_its_middle_unless_the_node_has_a_span() {
    let mut nodes = vec![
        cnode(120.0, 40.0, -1),
        cnode(60.0, 40.0, -1),
        cnode(60.0, 40.0, -1),
        cnode(60.0, 40.0, -1),
    ];
    seq(&mut nodes);
    let mut edges = vec![link(0, 1), link(0, 2), link(0, 3)];
    compound_layout(&mut nodes, &mut edges, Dir::TB, &OPTIONS);
    for e in &edges {
        assert!(close(e.route[0], nodes[0].x, 5));
    }
}

#[test]
fn compound_spreads_the_edges_on_one_side_of_a_node_in_the_order_of_the_nodes_they_lead_to() {
    for dir in DIRS {
        let mut nodes = vec![
            cnode(120.0, 120.0, -1),
            cnode(60.0, 60.0, -1),
            cnode(60.0, 60.0, -1),
            cnode(60.0, 60.0, -1),
        ];
        seq(&mut nodes);
        nodes[0].span = 96.0;
        let mut edges = vec![link(0, 1), link(0, 2), link(0, 3)];
        compound_layout(&mut nodes, &mut edges, dir, &OPTIONS);
        let across = if dir == Dir::TB || dir == Dir::BT {
            0
        } else {
            1
        };
        let starts: Vec<f64> = edges
            .iter()
            .map(|e| e.route[across] - if across == 0 { nodes[0].x } else { nodes[0].y })
            .collect();
        let targets: Vec<f64> = edges
            .iter()
            .map(|e| {
                if across == 0 {
                    nodes[e.dst].x
                } else {
                    nodes[e.dst].y
                }
            })
            .collect();
        let mut by_start = vec![0, 1, 2];
        by_start.sort_by(|&a, &b| starts[a].total_cmp(&starts[b]));
        let mut by_target = vec![0, 1, 2];
        by_target.sort_by(|&a, &b| targets[a].total_cmp(&targets[b]));
        assert_eq!(by_start, by_target, "{dir:?}");
        // None of the three can run straight, so they stay around the middle,
        // a pitch apart.
        assert!(close(starts[by_start[0]], -16.0, 5), "{dir:?}");
        assert!(close(starts[by_start[1]], 0.0, 5), "{dir:?}");
        assert!(close(starts[by_start[2]], 16.0, 5), "{dir:?}");
        for e in &edges {
            assert!(on_border(&nodes[0], e.route[0], e.route[1]), "{dir:?}");
        }
    }
}

#[test]
fn compound_keeps_an_edge_that_arrives_apart_from_one_that_leaves_on_the_same_side() {
    let mut nodes = vec![cnode(60.0, 40.0, -1), cnode(60.0, 40.0, -1)];
    seq(&mut nodes);
    spans(&mut nodes, 36.0);
    let mut edges = vec![link(0, 1), link(1, 0)];
    compound_layout(&mut nodes, &mut edges, Dir::TB, &OPTIONS);
    let (down, up) = (&edges[0].route, &edges[1].route);
    assert!(close((down[down.len() - 3] - up[0]).abs(), 16.0, 5));
    assert!(close((down[0] - up[up.len() - 3]).abs(), 16.0, 5));
}

#[test]
fn compound_shares_a_place_among_edges_that_run_the_same_way_when_there_is_no_room_for_one_each() {
    let mut nodes: Vec<CNode> = (0..=12).map(|_| cnode(60.0, 40.0, -1)).collect();
    seq(&mut nodes);
    spans(&mut nodes, 36.0);
    let mut edges: Vec<CEdge> = (0..12).map(|i| link(i, 12)).collect();
    // One edge the other way on the same side, which keeps a place of its
    // own.
    edges.push(link(12, 0));
    compound_layout(&mut nodes, &mut edges, Dir::TB, &OPTIONS);
    let into: Vec<f64> = edges[..12]
        .iter()
        .map(|e| e.route[e.route.len() - 3])
        .collect();
    let places: HashSet<i64> = into.iter().map(|&x| round(x * 100.0)).collect();
    assert!(places.len() <= 2);
    let back = &edges[12].route;
    assert!(close(back[1], nodes[12].y - 20.0, 5));
    for &x in &into {
        assert!((x - back[0]).abs() >= 8.0);
    }
}

#[test]
fn compound_stands_a_rank_that_only_sends_edges_on_one_line_and_hangs_a_rank_that_only_receives_them_from_one()
 {
    let lay = |span: f64| -> Vec<CNode> {
        let mut nodes = vec![
            cnode(60.0, 40.0, -1),
            cnode(60.0, 120.0, -1),
            cnode(60.0, 30.0, -1),
            cnode(60.0, 90.0, -1),
        ];
        seq(&mut nodes);
        spans(&mut nodes, span);
        compound_layout(
            &mut nodes,
            &mut [link(0, 2), link(1, 2), link(0, 3), link(1, 3)],
            Dir::TB,
            &OPTIONS,
        );
        nodes
    };
    let spread = lay(36.0);
    assert!(close(spread[0].y + 20.0, spread[1].y + 60.0, 5));
    assert!(close(spread[2].y - 15.0, spread[3].y - 45.0, 5));
    // Without spreading, as in the diagram types that do not use it yet, each
    // rank is centered.
    let plain = lay(0.0);
    assert!(close(plain[0].y, plain[1].y, 5));
    assert!(close(plain[2].y, plain[3].y, 5));
}

/// A node above a row of `children`, with an edge to each. With a `span`, the
/// edges spread along the nodes' sides.
fn fan(children: usize, span: f64) -> (Vec<CNode>, Vec<CEdge>, f64) {
    let mut nodes = vec![cnode(100.0, 40.0, -1)];
    let mut edges: Vec<CEdge> = Vec::new();
    for i in 1..=children {
        nodes.push(cnode(100.0, 40.0, -1));
        edges.push(link(0, i));
    }
    seq(&mut nodes);
    spans(&mut nodes, span);
    compound_layout(&mut nodes, &mut edges, Dir::TB, &OPTIONS);
    let gap = nodes[1].y - nodes[0].y - 40.0;
    (nodes, edges, gap)
}

#[test]
fn compound_gives_the_gap_between_two_ranks_more_room_when_edges_that_meet_at_the_middle_of_their_nodes_run_far_across_it()
 {
    assert!(close(fan(3, 0.0).2, OPTIONS.rank_sep, 5));
    // Seven children reach 420 across from the middle, and the gap is an
    // eighth of that.
    assert!(close(fan(7, 0.0).2, 420.0 / 8.0, 5));
    assert!(close(fan(40, 0.0).2, 3.0 * OPTIONS.rank_sep, 5));
}

#[test]
fn compound_turns_edges_that_run_far_across_side_by_side_at_levels_of_their_own_in_a_gap_that_grows_to_hold_them()
 {
    // With three children nothing runs far, and each edge is one curve across
    // the usual gap.
    let (_, few_edges, few_gap) = fan(3, 76.0);
    assert!(close(few_gap, OPTIONS.rank_sep, 5));
    for e in &few_edges {
        assert_eq!(e.route.len(), 6);
    }
    // With seven, the three on each side run down, across and down again.
    let (nodes, edges, gap) = fan(7, 76.0);
    let level = |e: &CEdge| -> f64 {
        assert_eq!(e.route.len(), 12);
        assert!(close(e.route[3], e.route[0], 5));
        assert!(close(e.route[7], e.route[4], 5));
        assert!(close(e.route[9], e.route[6], 5));
        e.route[4]
    };
    // The edge that runs furthest turns first, and each of the others 12
    // further down.
    assert!(close(level(&edges[1]) - level(&edges[0]), 12.0, 5));
    assert!(close(level(&edges[2]) - level(&edges[1]), 12.0, 5));
    for i in 0..3 {
        assert!(close(level(&edges[6 - i]), level(&edges[i]), 5));
    }
    // The one in the middle runs straight down.
    assert_eq!(edges[3].route.len(), 6);
    assert!(close(edges[3].route[3], edges[3].route[0], 5));
    // The levels keep 28 from the ranks above and below.
    assert!(close(level(&edges[0]) - nodes[0].y - 20.0, 28.0, 5));
    assert!(close(gap, 2.0 * 28.0 + 2.0 * 12.0, 5));
}

#[test]
fn compound_draws_the_edges_of_a_gap_as_plain_curves_in_a_taller_gap_where_they_cross_each_other_more_than_they_run_alongside()
 {
    // Four nodes above four, each with an edge to every one below.
    let mut nodes: Vec<CNode> = (0..8).map(|_| cnode(80.0, 40.0, -1)).collect();
    seq(&mut nodes);
    spans(&mut nodes, 56.0);
    let mut edges: Vec<CEdge> = Vec::new();
    for i in 0..4 {
        for j in 4..8 {
            edges.push(link(i, j));
        }
    }
    compound_layout(&mut nodes, &mut edges, Dir::TB, &OPTIONS);
    // No edge turns onto a level: each runs from one point to the other.
    for e in &edges {
        assert_eq!(e.route.len(), 6);
    }
    // The gap is half as tall as the widest edge runs across, and no more
    // than three times the usual.
    let widest = edges
        .iter()
        .map(|e| (e.route[0] - e.route[3]).abs())
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(widest > 6.0 * OPTIONS.rank_sep);
    assert!(close(
        nodes[4].y - nodes[0].y - 40.0,
        3.0 * OPTIONS.rank_sep,
        5
    ));
}

#[test]
fn compound_sets_a_rank_of_labels_half_an_arrowhead_nearer_the_tails_of_its_edges_and_holds_a_bend_beside_them_for_half_their_height()
 {
    let lay = |head_room: f64| -> Vec<CEdge> {
        let mut nodes = vec![cnode(60.0, 40.0, -1), cnode(60.0, 40.0, -1)];
        seq(&mut nodes);
        spans(&mut nodes, 36.0);
        let mut edges = vec![edge(0, 1, 40.0, 20.0), edge(0, 1, 50.0, 20.0), link(0, 1)];
        let options = LayeredOptions {
            head_room: Some(head_room),
            ..OPTIONS
        };
        compound_layout(&mut nodes, &mut edges, Dir::TB, &options);
        edges
    };
    // Without arrowheads the labels are midway between the two nodes.
    let plain = lay(0.0);
    assert!(close(plain[0].label_y, (40.0 + 108.0) / 2.0, 5));
    // With them, the labels are midway between the node above and the backs
    // of the arrowheads.
    let edges = lay(8.0);
    let (first, second, bare) = (&edges[0], &edges[1], &edges[2]);
    assert!(close(first.label_y, (40.0 + 108.0 - 8.0) / 2.0, 5));
    assert!(close(second.label_y, first.label_y, 5));
    // A labelled edge runs straight for the height of its label, and the bare
    // one for half of that.
    assert!(close(first.route[7] - first.route[4], 20.0, 5));
    assert!(close(bare.route[7] - bare.route[4], 10.0, 5));
    assert!(close(
        (bare.route[4] + bare.route[7]) / 2.0,
        first.label_y,
        5
    ));
}

#[test]
fn compound_turns_an_edge_that_runs_far_across_at_a_level_of_its_own_where_a_shorter_one_would_run_close_beside_it()
 {
    // A node above a row of three, and an edge back up to it from below the
    // row.
    let mut nodes: Vec<CNode> = (0..5).map(|_| cnode(80.0, 40.0, -1)).collect();
    seq(&mut nodes);
    spans(&mut nodes, 56.0);
    let mut edges = vec![
        link(0, 1),
        link(0, 2),
        link(0, 3),
        link(1, 4),
        link(2, 4),
        link(3, 4),
        link(4, 0),
    ];
    compound_layout(&mut nodes, &mut edges, Dir::TB, &OPTIONS);
    // The edge back up passes the row on one side, then runs across to the
    // top node and up into it.
    let back = &edges[6].route;
    let n = back.len();
    let level = back[n - 5];
    assert!(close(back[n - 8], level, 5));
    assert!(close(back[n - 6], back[n - 3], 5));
    assert!((back[n - 9] - back[n - 6]).abs() > 2.0 * OPTIONS.rank_sep);
    // The edge down to the node on that side leaves next to where it arrives.
    // It runs straight down as far as that level and curves below it.
    let down = &edges[2].route;
    assert!((down[0] - back[n - 3]).abs() < 20.0);
    assert_eq!(down.len(), 9);
    assert!(close(down[3], down[0], 5));
    assert!(down[4] > level - 0.01);
}

#[test]
fn compound_moves_an_end_along_its_side_to_let_its_edge_run_straight_and_no_closer_to_its_neighbour_than_a_pitch()
 {
    // A wide node above four narrow ones.
    let mut nodes = vec![
        cnode(400.0, 40.0, -1),
        cnode(40.0, 40.0, -1),
        cnode(40.0, 40.0, -1),
        cnode(40.0, 40.0, -1),
        cnode(40.0, 40.0, -1),
    ];
    seq(&mut nodes);
    for node in nodes.iter_mut() {
        node.span = node.w - 24.0;
    }
    let mut edges = vec![link(0, 1), link(0, 2), link(0, 3), link(0, 4)];
    compound_layout(&mut nodes, &mut edges, Dir::TB, &OPTIONS);
    // Each child is under the wide node, so each edge runs straight down to
    // it.
    for e in &edges {
        assert!(close(e.route[0], e.route[e.route.len() - 3], 5));
    }
    let mut two_way = vec![cnode(200.0, 40.0, -1), cnode(40.0, 40.0, -1)];
    seq(&mut two_way);
    for node in two_way.iter_mut() {
        node.span = node.w - 24.0;
    }
    let mut pair = vec![link(0, 1), link(1, 0)];
    compound_layout(&mut two_way, &mut pair, Dir::TB, &OPTIONS);
    assert!(close(
        (pair[0].route[0] - pair[1].route[pair[1].route.len() - 3]).abs(),
        16.0,
        5
    ));
}

#[test]
fn compound_moves_the_edges_that_leave_side_by_side_closer_together_before_it_lets_them_share_a_place()
 {
    let mut nodes = vec![cnode(60.0, 40.0, -1)];
    let mut edges: Vec<CEdge> = Vec::new();
    for i in 1..=7 {
        nodes.push(cnode(60.0, 40.0, -1));
        edges.push(link(0, i));
    }
    seq(&mut nodes);
    nodes[0].span = 36.0;
    compound_layout(&mut nodes, &mut edges, Dir::TB, &OPTIONS);
    let mut starts: Vec<f64> = edges.iter().map(|e| e.route[0]).collect();
    starts.sort_by(f64::total_cmp);
    // Six gaps of half a pitch each across the 36 there is.
    for i in 1..starts.len() {
        assert!(close(starts[i] - starts[i - 1], 6.0, 5));
    }
}

#[test]
fn compound_draws_an_edge_as_one_curve_across_a_rank_that_only_a_twin_of_another_edge_put_there() {
    let lay = |twin: bool| -> Vec<CEdge> {
        let mut nodes: Vec<CNode> = (0..4).map(|_| cnode(60.0, 40.0, -1)).collect();
        seq(&mut nodes);
        spans(&mut nodes, 36.0);
        let mut edges = vec![link(0, 1), link(0, 2), link(0, 3)];
        if twin {
            edges.push(link(0, 3));
        }
        compound_layout(&mut nodes, &mut edges, Dir::TB, &OPTIONS);
        edges
    };
    let plain = lay(false);
    let twinned = lay(true);
    // The twins make every edge take a rank in the middle, but none of them
    // needs a bend there.
    for e in &twinned[..2] {
        assert_eq!(e.route.len(), 6);
    }
    assert!(close(twinned[0].route[1], plain[0].route[1], 5));
    // The gap is tall enough for the twins to turn at two levels, 12 apart
    // and 28 from the ranks.
    assert!(close(
        twinned[0].route[4],
        plain[0].route[4] + 2.0 * 28.0 + 12.0 - OPTIONS.rank_sep,
        5
    ));
    // The twins leave and arrive at places of their own.
    let (one, other) = (&twinned[2].route, &twinned[3].route);
    assert!((one[0] - other[0]).abs() >= 8.0);
    assert!((one[one.len() - 3] - other[other.len() - 3]).abs() >= 8.0);
    // They would run close beside each other, so one turns straight away and
    // the other further down.
    let turns_at = |route: &[f64]| {
        if (route[3] - route[0]).abs() < 0.01 {
            route[4]
        } else {
            route[1]
        }
    };
    assert!((turns_at(one) - turns_at(other)).abs() >= 10.0);
}

#[test]
fn compound_keeps_twin_edges_apart_where_their_ends_cannot_be() {
    let mut nodes = vec![cnode(60.0, 40.0, -1), cnode(60.0, 40.0, -1)];
    seq(&mut nodes);
    let mut edges = vec![link(0, 1), link(0, 1)];
    compound_layout(&mut nodes, &mut edges, Dir::TB, &OPTIONS);
    let (a, b) = (&edges[0].route, &edges[1].route);
    assert_eq!(a.len(), 9);
    assert_eq!(b.len(), 9);
    assert!(close(a[0], b[0], 5));
    assert!(close((a[3] - b[3]).abs(), OPTIONS.edge_sep, 5));
}

/// A run across, with the x where the edge comes down to it and where it
/// goes down from it.
struct Run {
    y: f64,
    lo: f64,
    hi: f64,
    from: f64,
    to: f64,
}

#[test]
fn compound_routes_edges_at_right_angles_on_tracks_where_no_two_can_be_taken_for_each_other() {
    for seed in 1..=120u32 {
        let mut rnd = Random(seed * 7919);
        let mut nodes: Vec<CNode> = Vec::new();
        let count = 3 + rnd.int(14);
        for i in 0..count {
            let w = (40 + rnd.int(120)) as f64;
            let h = (30 + rnd.int(60)) as f64;
            let mut node = cnode(w, h, -1);
            node.seq = i as f64;
            node.span = node.w - 24.0;
            nodes.push(node);
        }
        let mut edges: Vec<CEdge> = Vec::new();
        let wanted = (rnd.next() * (count as f64 * 2.5)).floor() as usize;
        for _ in 0..wanted {
            let src = rnd.int(count);
            let dst = rnd.int(count);
            if src != dst {
                let label_w = if rnd.next() < 0.2 {
                    (30 + rnd.int(40)) as f64
                } else {
                    0.0
                };
                edges.push(edge(src, dst, label_w, 20.0));
            }
        }
        let options = LayeredOptions {
            tracks: true,
            ..OPTIONS
        };
        compound_layout(&mut nodes, &mut edges, Dir::TB, &options);
        let at = format!("seed {seed}");
        let mut runs: Vec<Run> = Vec::new();
        for (ei, e) in edges.iter().enumerate() {
            let r = &e.route;
            assert!(
                on_border(&nodes[e.src], r[0], r[1]),
                "{at} edge {ei} starts on its source"
            );
            assert!(
                on_border(&nodes[e.dst], r[r.len() - 3], r[r.len() - 2]),
                "{at} edge {ei} ends on its target"
            );
            for k in (3..r.len()).step_by(3) {
                // Two ends less than half a unit out of line are joined
                // directly.
                let across = (r[k + 1] - r[k - 2]).abs() < 0.01;
                let down = (r[k] - r[k - 3]).abs() < 0.5;
                assert!(across || down, "{at} edge {ei} turns at right angles");
                if !across || down {
                    continue;
                }
                // The route runs upwards when the edge does, so the higher
                // end is the one it comes down to.
                let before = if k >= 6 { r[k - 5] } else { r[k - 2] };
                let from_first =
                    before < r[k - 2] - 0.01 || (k + 4 < r.len() && r[k + 4] > r[k + 1] + 0.01);
                runs.push(Run {
                    y: r[k + 1],
                    lo: r[k].min(r[k - 3]),
                    hi: r[k].max(r[k - 3]),
                    from: if from_first { r[k - 3] } else { r[k] },
                    to: if from_first { r[k] } else { r[k - 3] },
                });
            }
        }
        for a in 0..runs.len() {
            for b in a + 1..runs.len() {
                if (runs[a].y - runs[b].y).abs() > 0.01 {
                    continue;
                }
                let overlap = runs[a].lo < runs[b].hi - 0.5 && runs[b].lo < runs[a].hi - 0.5;
                let mates = (runs[a].from - runs[b].from).abs() < 0.5
                    || (runs[a].to - runs[b].to).abs() < 0.5;
                assert!(
                    !(overlap && !mates),
                    "{at} runs {a} and {b} lie on one track"
                );
            }
        }
        // No run across passes through a node.
        for run in &runs {
            for node in &nodes {
                let inside = (run.y - node.y).abs() < node.h / 2.0 - 0.5
                    && run.lo < node.x + node.w / 2.0 - 0.5
                    && run.hi > node.x - node.w / 2.0 + 0.5;
                assert!(!inside, "{at} a run crosses a node");
            }
        }
    }
}

#[test]
fn compound_nests_the_tracks_of_a_fan_so_that_its_edges_do_not_cross() {
    let mut nodes = vec![
        cnode(120.0, 40.0, -1),
        cnode(60.0, 40.0, -1),
        cnode(60.0, 40.0, -1),
        cnode(60.0, 40.0, -1),
        cnode(60.0, 40.0, -1),
    ];
    seq(&mut nodes);
    nodes[0].span = 96.0;
    let mut edges = vec![link(0, 1), link(0, 2), link(0, 3), link(0, 4)];
    let options = LayeredOptions {
        tracks: true,
        ..OPTIONS
    };
    compound_layout(&mut nodes, &mut edges, Dir::TB, &options);
    let across_y = |e: &CEdge| e.route[4];
    let mut left: Vec<&CEdge> = edges
        .iter()
        .filter(|e| nodes[e.dst].x < nodes[0].x)
        .collect();
    left.sort_by(|a, b| nodes[a.dst].x.total_cmp(&nodes[b.dst].x));
    let mut right: Vec<&CEdge> = edges
        .iter()
        .filter(|e| nodes[e.dst].x > nodes[0].x)
        .collect();
    right.sort_by(|a, b| nodes[b.dst].x.total_cmp(&nodes[a.dst].x));
    // The edge that reaches furthest turns first, so the nearer ones pass
    // under it.
    for side in [left, right] {
        for i in 1..side.len() {
            assert!(across_y(side[i - 1]) < across_y(side[i]));
        }
    }
}

#[test]
fn compound_survives_groups_that_contain_each_other() {
    let mut a = cnode(0.0, 0.0, 1);
    let mut b = cnode(0.0, 0.0, 0);
    a.is_group = true;
    b.is_group = true;
    let leaf = cnode(40.0, 40.0, 0);
    let mut nodes = vec![a, b, leaf, cnode(40.0, 40.0, -1)];
    let mut edges = vec![link(2, 3)];
    let size = compound_layout(&mut nodes, &mut edges, Dir::TB, &OPTIONS);
    assert!(size.width.is_finite());
    assert!(edges[0].route.len() >= 6);
}

fn group(parent: i32) -> CNode {
    let mut node = cnode(0.0, 0.0, parent);
    node.is_group = true;
    node.pad_x = 20.0;
    node.pad_top = 36.0;
    node.pad_bottom = 20.0;
    node.min_w = 60.0;
    node
}

struct Sample {
    nodes: Vec<CNode>,
    edges: Vec<CEdge>,
    lanes: Vec<usize>,
    leaves: Vec<usize>,
    lane_of: Vec<usize>,
    inner: Vec<usize>,
    inner_lane: Vec<usize>,
    dir: Dir,
}

fn lane_graph(seed: u32) -> Sample {
    let mut rnd = Random(seed);
    let mut nodes: Vec<CNode> = Vec::new();
    let mut leaves: Vec<usize> = Vec::new();
    let mut lanes: Vec<usize> = Vec::new();
    let leaf_count = 1 + rnd.int(30);
    let lane_count = 1 + rnd.int(5);
    let mut lane_of = vec![0usize; leaf_count];
    let mut inner: Vec<usize> = Vec::new();
    let mut inner_lane: Vec<usize> = Vec::new();
    // Leaves first, lanes after them, as the flowchart graph lists them.
    for i in 0..leaf_count {
        let w = (30 + rnd.int(120)) as f64;
        let h = (24 + rnd.int(60)) as f64;
        nodes.push(cnode(w, h, -1));
        leaves.push(i);
    }
    for _ in 0..lane_count {
        lanes.push(nodes.len());
        nodes.push(group(-1));
    }
    // The bound is drawn again before every turn, as Pele's loop does.
    let mut k = 0;
    while k < rnd.int(4) {
        let lane = lanes[rnd.int(lane_count)];
        inner.push(nodes.len());
        inner_lane.push(lane);
        nodes.push(group(lane as i32));
        k += 1;
    }
    for &i in &leaves {
        if !inner.is_empty() && rnd.next() < 0.25 {
            let g = rnd.int(inner.len());
            nodes[i].parent = inner[g] as i32;
            lane_of[i] = inner_lane[g];
        } else {
            lane_of[i] = lanes[rnd.int(lane_count)];
            nodes[i].parent = lane_of[i] as i32;
        }
    }
    let mut edges: Vec<CEdge> = Vec::new();
    let count = rnd.int(leaf_count * 2);
    for _ in 0..count {
        let labelled = rnd.next() < 0.3;
        let src = rnd.int(leaf_count);
        let dst = rnd.int(leaf_count);
        let label_w = if labelled {
            (20 + rnd.int(80)) as f64
        } else {
            0.0
        };
        edges.push(edge(src, dst, label_w, if labelled { 18.0 } else { 0.0 }));
    }
    if rnd.next() < 0.3 {
        edges.push(link(lanes[0], leaves[0]));
    }
    let dir = DIRS[rnd.int(4)];
    Sample {
        nodes,
        edges,
        lanes,
        leaves,
        lane_of,
        inner,
        inner_lane,
        dir,
    }
}

#[test]
fn lanes_lay_out_nothing() {
    let result = lane_layout(&mut [], &mut [], Dir::TB, &OPTIONS, None);
    assert_eq!((result.width, result.height), (0.0, 0.0));
}

#[test]
fn lanes_sit_side_by_side_across_the_flow_in_the_order_written() {
    for dir in DIRS {
        let mut nodes = vec![
            cnode(60.0, 30.0, -1),
            cnode(60.0, 30.0, -1),
            group(-1),
            group(-1),
        ];
        nodes[0].parent = 3;
        nodes[1].parent = 2;
        let result = lane_layout(&mut nodes, &mut [link(0, 1)], dir, &OPTIONS, None);
        let (first, second) = (&nodes[3], &nodes[2]);
        if dir == Dir::TB || dir == Dir::BT {
            assert!(first.x < second.x, "{dir:?}");
            assert_eq!(first.h, result.height, "{dir:?}");
            assert!(close(first.w + second.w, result.width, 6), "{dir:?}");
        } else {
            assert!(first.y < second.y, "{dir:?}");
            assert_eq!(first.w, result.width, "{dir:?}");
            assert!(close(first.h + second.h, result.height, 6), "{dir:?}");
        }
    }
}

#[test]
fn lanes_run_the_flow_in_the_direction_asked_for() {
    let at = |dir: Dir| -> Vec<CNode> {
        let mut nodes = vec![cnode(60.0, 30.0, 2), cnode(60.0, 30.0, 2), group(-1)];
        lane_layout(&mut nodes, &mut [link(0, 1)], dir, &OPTIONS, None);
        nodes
    };
    assert!(at(Dir::TB)[0].y < at(Dir::TB)[1].y);
    assert!(at(Dir::BT)[0].y > at(Dir::BT)[1].y);
    assert!(at(Dir::LR)[0].x < at(Dir::LR)[1].x);
    assert!(at(Dir::RL)[0].x > at(Dir::RL)[1].x);
}

#[test]
fn lanes_keep_a_chain_in_one_lane_on_a_straight_line() {
    let mut nodes = vec![
        cnode(50.0, 30.0, 3),
        cnode(120.0, 30.0, 3),
        cnode(70.0, 30.0, 3),
        group(-1),
    ];
    let mut edges = vec![link(0, 1), link(1, 2)];
    lane_layout(&mut nodes, &mut edges, Dir::TB, &OPTIONS, None);
    assert!(close(nodes[0].x, nodes[1].x, 6));
    assert!(close(nodes[1].x, nodes[2].x, 6));
}

#[test]
fn lanes_separate_an_approval_decisions_incoming_accepted_and_returned_paths() {
    for dir in DIRS {
        let mut nodes = vec![
            cnode(60.0, 40.0, 8),
            cnode(80.0, 40.0, 7),
            cnode(70.0, 70.0, 7),
            cnode(80.0, 40.0, 6),
            cnode(80.0, 40.0, 6),
            cnode(60.0, 40.0, 8),
            group(-1),
            group(-1),
            group(-1),
        ];
        let mut edges = vec![
            edge(0, 1, 70.0, 20.0),
            link(1, 2),
            edge(2, 3, 50.0, 20.0),
            link(3, 4),
            link(4, 5),
            edge(2, 0, 70.0, 20.0),
        ];
        lane_layout(&mut nodes, &mut edges, dir, &OPTIONS, None);
        let incoming = end_side(&edges[1].route, edges[1].route.len() - 3, &nodes[2]);
        let accepted = end_side(&edges[2].route, 0, &nodes[2]);
        let returned = end_side(&edges[5].route, 0, &nodes[2]);
        let sides: HashSet<i32> = [incoming, accepted, returned].into_iter().collect();
        assert_eq!(sides.len(), 3, "{dir:?}");
        assert!(!sides.contains(&-1), "{dir:?}");
        let back = &edges[5];
        let mut corners = 0;
        for p in (6..back.route.len()).step_by(3) {
            if (back.route[p] == back.route[p - 3]) != (back.route[p - 3] == back.route[p - 6]) {
                corners += 1;
            }
        }
        assert_eq!(corners, 1, "{dir:?}");
        for p in (3..back.route.len()).step_by(3) {
            assert!(
                back.route[p] == back.route[p - 3] || back.route[p + 1] == back.route[p - 2],
                "{dir:?}"
            );
        }
        for node in &nodes[..6] {
            let [l, t, r, b] = boxed(node);
            let clear = back.label_x + back.label_w / 2.0 <= l
                || back.label_x - back.label_w / 2.0 >= r
                || back.label_y + back.label_h / 2.0 <= t
                || back.label_y - back.label_h / 2.0 >= b;
            assert!(clear, "{dir:?}");
        }
    }
}

#[test]
fn lanes_keep_a_handoff_into_a_step_apart_from_the_handoff_out_of_it() {
    for dir in DIRS {
        let mut nodes = vec![
            cnode(110.0, 38.0, 5),
            cnode(110.0, 38.0, 5),
            cnode(100.0, 38.0, 4),
            cnode(110.0, 38.0, 4),
            group(-1),
            group(-1),
        ];
        let mut edges = vec![link(2, 3), link(0, 3), link(3, 1)];
        lane_layout(&mut nodes, &mut edges, dir, &OPTIONS, None);
        let (into, out) = (&edges[1].route, &edges[2].route);
        assert_ne!(
            end_side(into, into.len() - 3, &nodes[3]),
            end_side(out, 0, &nodes[3]),
            "{dir:?}"
        );
        for p in (3..into.len()).step_by(3) {
            for q in (3..out.len()).step_by(3) {
                let upright = into[p] == into[p - 3] && out[q] == out[q - 3] && into[p] == out[q];
                let level = into[p + 1] == into[p - 2]
                    && out[q + 1] == out[q - 2]
                    && into[p + 1] == out[q + 1];
                let k = if upright { 1 } else { 0 };
                let shared = into[p + k]
                    .max(into[p - 3 + k])
                    .min(out[q + k].max(out[q - 3 + k]))
                    - into[p + k]
                        .min(into[p - 3 + k])
                        .max(out[q + k].min(out[q - 3 + k]));
                if upright || level {
                    assert!(shared <= 0.0, "{dir:?}");
                }
            }
        }
    }
}

#[test]
fn lanes_take_a_return_past_the_steps_between_around_the_margin_of_its_lane() {
    for dir in DIRS {
        let mut nodes = vec![
            cnode(140.0, 44.0, 8),
            cnode(140.0, 44.0, 8),
            cnode(140.0, 44.0, 7),
            cnode(120.0, 76.0, 7),
            cnode(140.0, 44.0, 6),
            cnode(140.0, 44.0, 6),
            group(-1),
            group(-1),
            group(-1),
        ];
        let mut edges = vec![
            link(0, 1),
            link(1, 2),
            link(2, 3),
            edge(3, 4, 30.0, 20.0),
            link(4, 5),
            edge(3, 0, 30.0, 20.0),
        ];
        lane_layout(&mut nodes, &mut edges, dir, &OPTIONS, None);
        let sideways = dir == Dir::LR || dir == Dir::RL;
        let across = |c: &CNode| if sideways { c.y } else { c.x };
        let along = |c: &CNode| if sideways { c.x } else { c.y };
        let depth = |c: &CNode| if sideways { c.w } else { c.h };
        assert!(close(across(&nodes[0]), across(&nodes[1]), 6), "{dir:?}");
        assert!(close(across(&nodes[4]), across(&nodes[5]), 6), "{dir:?}");
        let back = &edges[5].route;
        let sides: HashSet<i32> = [
            end_side(&edges[2].route, edges[2].route.len() - 3, &nodes[3]),
            end_side(&edges[3].route, 0, &nodes[3]),
            end_side(back, 0, &nodes[3]),
        ]
        .into_iter()
        .collect();
        assert_eq!(sides.len(), 3, "{dir:?}");
        assert_ne!(
            end_side(back, back.len() - 3, &nodes[0]),
            end_side(&edges[0].route, 0, &nodes[0]),
            "{dir:?}"
        );
        for p in (3..back.len()).step_by(3) {
            assert!(
                back[p] == back[p - 3] || back[p + 1] == back[p - 2],
                "{dir:?}"
            );
            for (i, node) in nodes[..6].iter().enumerate() {
                if i == 0 || i == 3 {
                    continue;
                }
                let [l, t, r, b] = boxed(node);
                assert!(
                    back[p].max(back[p - 3]) <= l
                        || back[p].min(back[p - 3]) >= r
                        || back[p + 1].max(back[p - 2]) <= t
                        || back[p + 1].min(back[p - 2]) >= b,
                    "{dir:?}"
                );
            }
        }
        let gap = |a: usize, b: usize| {
            (along(&nodes[b]) - along(&nodes[a])).abs()
                - (depth(&nodes[a]) + depth(&nodes[b])) / 2.0
        };
        assert!(gap(1, 2) < gap(0, 1), "{dir:?}");
        assert!(close(gap(0, 1), OPTIONS.rank_sep, 6), "{dir:?}");
    }
}

#[test]
fn lanes_draw_a_link_asked_to_be_longer_at_its_length_beside_a_handoff() {
    for dir in DIRS {
        let mut nodes = vec![
            cnode(80.0, 40.0, 5),
            cnode(80.0, 40.0, 5),
            cnode(80.0, 40.0, 5),
            cnode(80.0, 40.0, 4),
            group(-1),
            group(-1),
        ];
        let mut edges = vec![
            CEdge {
                minlen: 2,
                ..link(0, 1)
            },
            link(1, 2),
            link(2, 3),
        ];
        lane_layout(&mut nodes, &mut edges, dir, &OPTIONS, None);
        let sideways = dir == Dir::LR || dir == Dir::RL;
        let gap = |a: usize, b: usize| {
            (if sideways {
                nodes[b].x - nodes[a].x
            } else {
                nodes[b].y - nodes[a].y
            })
            .abs()
                - if sideways { 80.0 } else { 40.0 }
        };
        assert!(close(gap(0, 1), 2.0 * OPTIONS.rank_sep, 6), "{dir:?}");
        assert!(close(gap(1, 2), OPTIONS.rank_sep, 6), "{dir:?}");
    }
}

#[test]
fn lanes_route_a_handoff_by_where_the_label_of_a_relation_to_a_lane_is() {
    let mut nodes = vec![
        cnode(110.0, 38.0, 7),
        cnode(110.0, 38.0, 6),
        cnode(110.0, 38.0, 6),
        cnode(110.0, 38.0, 6),
        cnode(110.0, 38.0, 6),
        cnode(110.0, 38.0, 6),
        group(-1),
        group(-1),
    ];
    let mut edges = vec![
        link(0, 1),
        link(1, 2),
        link(2, 3),
        link(3, 4),
        link(4, 5),
        edge(6, 5, 400.0, 200.0),
    ];
    lane_layout(&mut nodes, &mut edges, Dir::TB, &OPTIONS, None);
    let handoff = &edges[0].route;
    assert!(handoff.len() > 6);
    for p in (3..handoff.len()).step_by(3) {
        assert!(handoff[p] == handoff[p - 3] || handoff[p + 1] == handoff[p - 2]);
    }
    for p in (1..handoff.len()).step_by(3) {
        assert!((handoff[p] - edges[5].label_y).abs() > 100.0);
    }
}

fn lanes_keep_random_graphs_well_formed(lane_pad: f64) {
    for seed in 1..=300 {
        let Sample {
            mut nodes,
            mut edges,
            lanes,
            leaves,
            lane_of,
            inner,
            inner_lane,
            dir,
        } = lane_graph(seed);
        let result = lane_layout(&mut nodes, &mut edges, dir, &OPTIONS, Some(lane_pad));
        let at = format!("seed {seed}");
        assert!(
            result.width.is_finite() && result.height.is_finite(),
            "{at}"
        );
        for c in &nodes {
            assert!(
                c.x.is_finite() && c.y.is_finite() && c.w.is_finite() && c.h.is_finite(),
                "{at}"
            );
        }
        for &i in &leaves {
            let [l, t, r, b] = boxed(&nodes[i]);
            let [ll, lt, lr, lb] = boxed(&nodes[lane_of[i]]);
            assert!(
                l >= ll - 0.01 && r <= lr + 0.01 && t >= lt - 0.01 && b <= lb + 0.01,
                "{at}: node {i} in its lane"
            );
        }
        for (k, &g) in inner.iter().enumerate() {
            if !leaves.iter().any(|&i| nodes[i].parent == g as i32) {
                continue;
            }
            let [l, t, r, b] = boxed(&nodes[g]);
            let [ll, lt, lr, lb] = boxed(&nodes[inner_lane[k]]);
            assert!(
                l >= ll - 0.01 && r <= lr + 0.01 && t >= lt - 0.01 && b <= lb + 0.01,
                "{at}: group {g} in its lane"
            );
            for &i in &leaves {
                if nodes[i].parent != g as i32 {
                    continue;
                }
                let [nl, nt, nr, nb] = boxed(&nodes[i]);
                assert!(
                    nl >= l - 0.01 && nr <= r + 0.01 && nt >= t - 0.01 && nb <= b + 0.01,
                    "{at}: node {i} in its group"
                );
            }
        }
        for a in 0..leaves.len() {
            for b in a + 1..leaves.len() {
                let [al, at_, ar, ab] = boxed(&nodes[leaves[a]]);
                let [bl, bt, br, bb] = boxed(&nodes[leaves[b]]);
                let apart =
                    ar <= bl + 0.01 || br <= al + 0.01 || ab <= bt + 0.01 || bb <= at_ + 0.01;
                assert!(apart, "{at}: nodes {a} and {b} overlap");
            }
        }
        for a in 0..lanes.len() {
            let [l, t, r, b] = boxed(&nodes[lanes[a]]);
            assert!(
                l >= -0.01 && t >= -0.01 && r <= result.width + 0.01 && b <= result.height + 0.01,
                "{at}: lane inside"
            );
            for &other in &lanes[a + 1..] {
                let [ol, ot, or, ob] = boxed(&nodes[other]);
                assert!(
                    r <= ol + 0.01 || or <= l + 0.01 || b <= ot + 0.01 || ob <= t + 0.01,
                    "{at}: lanes overlap"
                );
            }
        }
        for e in &edges {
            if e.src == e.dst {
                continue;
            }
            assert!(
                e.route.len() >= 6 && e.route.len().is_multiple_of(3),
                "{at}: route"
            );
            for v in &e.route {
                assert!(v.is_finite(), "{at}: route value");
            }
            for p in (0..e.route.len()).step_by(3) {
                let inside = e.route[p] >= -0.01
                    && e.route[p] <= result.width + 0.01
                    && e.route[p + 1] >= -0.01
                    && e.route[p + 1] <= result.height + 0.01;
                assert!(inside, "{at}: route inside the lanes");
            }
            assert!(
                e.label_x.is_finite() && e.label_y.is_finite(),
                "{at}: label"
            );
        }
    }
}

#[test]
fn lanes_keep_random_graphs_well_formed_with_24_pixels_of_lane_padding() {
    lanes_keep_random_graphs_well_formed(24.0);
}

#[test]
fn lanes_keep_random_graphs_well_formed_with_12_pixels_of_lane_padding() {
    lanes_keep_random_graphs_well_formed(12.0);
}

#[test]
fn lanes_give_the_same_result_every_time() {
    let mut a = lane_graph(42);
    let mut b = lane_graph(42);
    lane_layout(&mut a.nodes, &mut a.edges, a.dir, &OPTIONS, None);
    lane_layout(&mut b.nodes, &mut b.edges, b.dir, &OPTIONS, None);
    let shape = |nodes: &[CNode]| {
        nodes
            .iter()
            .map(|c| [c.x, c.y, c.w, c.h])
            .collect::<Vec<_>>()
    };
    assert_eq!(shape(&a.nodes), shape(&b.nodes));
    let routes = |edges: &[CEdge]| edges.iter().map(|e| e.route.clone()).collect::<Vec<_>>();
    assert_eq!(routes(&a.edges), routes(&b.edges));
}
