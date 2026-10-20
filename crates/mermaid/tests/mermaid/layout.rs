//! The compound and lane layouts: Pele's tests of them, case for case.

use std::collections::HashSet;

use mermaid::{
    geom::{Axis, Direction, Point, RoutePoint, Side, Size},
    layout::{
        compound::{CEdge, CNode, compound_layout},
        lanes::lane_layout,
        layered::LayeredOptions,
    },
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
const DIRS: [Direction; 4] = [
    Direction::TopToBottom,
    Direction::BottomToTop,
    Direction::LeftToRight,
    Direction::RightToLeft,
];

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
        label: Size::new(label_w, label_h),
        ..CEdge::default()
    }
}

fn link(src: usize, dst: usize) -> CEdge {
    edge(src, dst, 0.0, 0.0)
}

fn on_border(node: &CNode, p: Point) -> bool {
    let dx = (p.x - node.center.x).abs();
    let dy = (p.y - node.center.y).abs();
    let inside = dx <= node.size.w / 2.0 + 0.5 && dy <= node.size.h / 2.0 + 0.5;
    let edge_x = (dx - node.size.w / 2.0).abs() <= 0.5;
    let edge_y = (dy - node.size.h / 2.0).abs() <= 0.5;
    inside && (edge_x || edge_y)
}

/// The side of `node` that point `at` of `route` lies on.
fn end_side(route: &[RoutePoint], at: usize, node: &CNode) -> Option<Side> {
    node.side_of(route[at].at)
}

fn is_ancestor(nodes: &[CNode], a: usize, b: usize) -> bool {
    let mut p = nodes[b].parent;
    while let Some(at) = p {
        if at == a {
            return true;
        }
        p = nodes[at].parent;
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
        c.center.x - c.size.w / 2.0,
        c.center.y - c.size.h / 2.0,
        c.center.x + c.size.w / 2.0,
        c.center.y + c.size.h / 2.0,
    ]
}

/// Every other graph lets edges spread along the sides of its nodes.
fn compound_graph(seed: u32) -> (Vec<CNode>, Vec<CEdge>, Direction) {
    let mut draws = Random(seed);
    let mut nodes: Vec<CNode> = Vec::new();
    let mut groups: Vec<usize> = Vec::new();
    let group_count = draws.int(5);
    for g in 0..group_count {
        let parent = if !groups.is_empty() && draws.next() < 0.4 {
            Some(groups[draws.int(groups.len())])
        } else {
            None
        };
        let mut node = CNode::new(Size::default(), parent);
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
            Some(groups[draws.int(groups.len())])
        } else {
            None
        };
        let w = (30 + draws.int(120)) as f64;
        let h = (30 + draws.int(50)) as f64;
        let mut node = CNode::new(Size::new(w, h), parent);
        node.seq = i as f64;
        if seed.is_multiple_of(2) {
            node.span = node.size.w.min(node.size.h) - 24.0;
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

        assert!(size.w.is_finite() && size.h.is_finite(), "{at}");
        for (i, node) in nodes.iter().enumerate() {
            assert!(
                node.center.x.is_finite() && node.center.y.is_finite(),
                "{at} node {i}"
            );
            assert!(
                node.center.x - node.size.w / 2.0 >= -0.5,
                "{at} node {i} left"
            );
            assert!(
                node.center.y - node.size.h / 2.0 >= -0.5,
                "{at} node {i} top"
            );
            assert!(
                node.center.x + node.size.w / 2.0 <= size.w + 0.5,
                "{at} node {i} right"
            );
            assert!(
                node.center.y + node.size.h / 2.0 <= size.h + 0.5,
                "{at} node {i} bottom"
            );
            if let Some(parent) = node.parent {
                let parent = &nodes[parent];
                assert!(
                    (node.center.x - parent.center.x).abs() + node.size.w / 2.0
                        <= parent.size.w / 2.0 + 0.5,
                    "{at} node {i} in parent"
                );
                assert!(
                    (node.center.y - parent.center.y).abs() + node.size.h / 2.0
                        <= parent.size.h / 2.0 + 0.5,
                    "{at} node {i} in parent"
                );
            }
        }

        for a in 0..nodes.len() {
            for b in a + 1..nodes.len() {
                if nodes[a].parent != nodes[b].parent {
                    continue;
                }
                let overlap_x = (nodes[a].size.w + nodes[b].size.w) / 2.0
                    - (nodes[a].center.x - nodes[b].center.x).abs();
                let overlap_y = (nodes[a].size.h + nodes[b].size.h) / 2.0
                    - (nodes[a].center.y - nodes[b].center.y).abs();
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
            assert!(e.route.len() >= 2, "{at} edge {i} has a route");
            for point in &e.route {
                assert!(
                    point.at.x.is_finite() && point.at.y.is_finite(),
                    "{at} edge {i}"
                );
            }
            if is_ancestor(&nodes, e.src, e.dst) || is_ancestor(&nodes, e.dst, e.src) {
                continue;
            }
            let last = e.route.len() - 1;
            assert!(
                on_border(&nodes[e.src], e.route[0].at),
                "{at} edge {i} starts on its source"
            );
            assert!(
                on_border(&nodes[e.dst], e.route[last].at),
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
                .map(|n| [n.center.x, n.center.y, n.size.w, n.size.h])
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
            CNode::new(Size::new(60.0, 40.0), None),
            CNode::new(Size::new(60.0, 40.0), None),
            CNode::new(Size::new(60.0, 40.0), None),
        ];
        seq(&mut nodes);
        compound_layout(&mut nodes, &mut [link(0, 1), link(1, 2)], dir, &OPTIONS);
        let vertical = dir.is_vertical();
        let axis = |n: &CNode| if vertical { n.center.y } else { n.center.x };
        let sign = if dir == Direction::TopToBottom || dir == Direction::LeftToRight {
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
        CNode::new(Size::new(60.0, 40.0), None),
        CNode::new(Size::new(60.0, 40.0), None),
        CNode::new(Size::new(60.0, 40.0), None),
    ];
    seq(&mut nodes);
    let mut edges = vec![link(0, 1), link(1, 2), link(0, 2)];
    compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &OPTIONS);
    let route = &edges[2].route;
    let mut xs = HashSet::new();
    for point in &route[1..route.len() - 1] {
        xs.insert(round(point.at.x));
    }
    assert_eq!(xs.len(), 1);
}

#[test]
fn compound_runs_an_edge_that_skips_ranks_in_line_with_one_of_its_ends_so_that_it_turns_once_at_the_other()
 {
    // A chain of eight, with edges that skip ranks down it and back up.
    let mut nodes: Vec<CNode> = (0..8)
        .map(|_| CNode::new(Size::new(80.0, 40.0), None))
        .collect();
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
    compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &OPTIONS);
    // How many times the route changes its place across the flow.
    let turns = |route: &[RoutePoint]| {
        route
            .windows(2)
            .filter(|pair| (pair[1].at.x - pair[0].at.x).abs() > 0.01)
            .count()
    };
    for e in &edges[7..] {
        assert_eq!(turns(&e.route), 1, "{} to {}", e.src, e.dst);
    }
    // The edge from the second node down and the one back up to it both meet
    // it in line.
    let down = &edges[9].route;
    let up = &edges[11].route;
    assert!(close(down[1].at.x, down[0].at.x, 5));
    assert!(close(up[up.len() - 2].at.x, up[up.len() - 1].at.x, 5));
}

#[test]
fn compound_keeps_the_edges_on_one_side_of_a_node_at_its_middle_unless_the_node_has_a_span() {
    let mut nodes = vec![
        CNode::new(Size::new(120.0, 40.0), None),
        CNode::new(Size::new(60.0, 40.0), None),
        CNode::new(Size::new(60.0, 40.0), None),
        CNode::new(Size::new(60.0, 40.0), None),
    ];
    seq(&mut nodes);
    let mut edges = vec![link(0, 1), link(0, 2), link(0, 3)];
    compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &OPTIONS);
    for e in &edges {
        assert!(close(e.route[0].at.x, nodes[0].center.x, 5));
    }
}

#[test]
fn compound_spreads_the_edges_on_one_side_of_a_node_in_the_order_of_the_nodes_they_lead_to() {
    for dir in DIRS {
        let mut nodes = vec![
            CNode::new(Size::new(120.0, 120.0), None),
            CNode::new(Size::new(60.0, 60.0), None),
            CNode::new(Size::new(60.0, 60.0), None),
            CNode::new(Size::new(60.0, 60.0), None),
        ];
        seq(&mut nodes);
        nodes[0].span = 96.0;
        let mut edges = vec![link(0, 1), link(0, 2), link(0, 3)];
        compound_layout(&mut nodes, &mut edges, dir, &OPTIONS);
        let across = if dir.is_vertical() { Axis::X } else { Axis::Y };
        let starts: Vec<f64> = edges
            .iter()
            .map(|e| e.route[0].at.along(across) - nodes[0].center.along(across))
            .collect();
        let targets: Vec<f64> = edges
            .iter()
            .map(|e| nodes[e.dst].center.along(across))
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
            assert!(on_border(&nodes[0], e.route[0].at), "{dir:?}");
        }
    }
}

#[test]
fn compound_keeps_an_edge_that_arrives_apart_from_one_that_leaves_on_the_same_side() {
    let mut nodes = vec![
        CNode::new(Size::new(60.0, 40.0), None),
        CNode::new(Size::new(60.0, 40.0), None),
    ];
    seq(&mut nodes);
    spans(&mut nodes, 36.0);
    let mut edges = vec![link(0, 1), link(1, 0)];
    compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &OPTIONS);
    let (down, up) = (&edges[0].route, &edges[1].route);
    assert!(close(
        (down[down.len() - 1].at.x - up[0].at.x).abs(),
        16.0,
        5
    ));
    assert!(close((down[0].at.x - up[up.len() - 1].at.x).abs(), 16.0, 5));
}

#[test]
fn compound_shares_a_place_among_edges_that_run_the_same_way_when_there_is_no_room_for_one_each() {
    let mut nodes: Vec<CNode> = (0..=12)
        .map(|_| CNode::new(Size::new(60.0, 40.0), None))
        .collect();
    seq(&mut nodes);
    spans(&mut nodes, 36.0);
    let mut edges: Vec<CEdge> = (0..12).map(|i| link(i, 12)).collect();
    // One edge the other way on the same side, which keeps a place of its
    // own.
    edges.push(link(12, 0));
    compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &OPTIONS);
    let into: Vec<f64> = edges[..12]
        .iter()
        .map(|e| e.route[e.route.len() - 1].at.x)
        .collect();
    let places: HashSet<i64> = into.iter().map(|&x| round(x * 100.0)).collect();
    assert!(places.len() <= 2);
    let back = &edges[12].route;
    assert!(close(back[0].at.y, nodes[12].center.y - 20.0, 5));
    for &x in &into {
        assert!((x - back[0].at.x).abs() >= 8.0);
    }
}

#[test]
fn compound_stands_a_rank_that_only_sends_edges_on_one_line_and_hangs_a_rank_that_only_receives_them_from_one()
 {
    let lay = |span: f64| -> Vec<CNode> {
        let mut nodes = vec![
            CNode::new(Size::new(60.0, 40.0), None),
            CNode::new(Size::new(60.0, 120.0), None),
            CNode::new(Size::new(60.0, 30.0), None),
            CNode::new(Size::new(60.0, 90.0), None),
        ];
        seq(&mut nodes);
        spans(&mut nodes, span);
        compound_layout(
            &mut nodes,
            &mut [link(0, 2), link(1, 2), link(0, 3), link(1, 3)],
            Direction::TopToBottom,
            &OPTIONS,
        );
        nodes
    };
    let spread = lay(36.0);
    assert!(close(
        spread[0].center.y + 20.0,
        spread[1].center.y + 60.0,
        5
    ));
    assert!(close(
        spread[2].center.y - 15.0,
        spread[3].center.y - 45.0,
        5
    ));
    // Without spreading, as in the diagram types that do not use it yet, each
    // rank is centered.
    let plain = lay(0.0);
    assert!(close(plain[0].center.y, plain[1].center.y, 5));
    assert!(close(plain[2].center.y, plain[3].center.y, 5));
}

/// A node above a row of `children`, with an edge to each. With a `span`, the
/// edges spread along the nodes' sides.
fn fan(children: usize, span: f64) -> (Vec<CNode>, Vec<CEdge>, f64) {
    let mut nodes = vec![CNode::new(Size::new(100.0, 40.0), None)];
    let mut edges: Vec<CEdge> = Vec::new();
    for i in 1..=children {
        nodes.push(CNode::new(Size::new(100.0, 40.0), None));
        edges.push(link(0, i));
    }
    seq(&mut nodes);
    spans(&mut nodes, span);
    compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &OPTIONS);
    let gap = nodes[1].center.y - nodes[0].center.y - 40.0;
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
        assert_eq!(e.route.len(), 2);
    }
    // With seven, the three on each side run down, across and down again.
    let (nodes, edges, gap) = fan(7, 76.0);
    let level = |e: &CEdge| -> f64 {
        assert_eq!(e.route.len(), 4);
        assert!(close(e.route[1].at.x, e.route[0].at.x, 5));
        assert!(close(e.route[2].at.y, e.route[1].at.y, 5));
        assert!(close(e.route[3].at.x, e.route[2].at.x, 5));
        e.route[1].at.y
    };
    // The edge that runs furthest turns first, and each of the others 12
    // further down.
    assert!(close(level(&edges[1]) - level(&edges[0]), 12.0, 5));
    assert!(close(level(&edges[2]) - level(&edges[1]), 12.0, 5));
    for i in 0..3 {
        assert!(close(level(&edges[6 - i]), level(&edges[i]), 5));
    }
    // The one in the middle runs straight down.
    assert_eq!(edges[3].route.len(), 2);
    assert!(close(edges[3].route[1].at.x, edges[3].route[0].at.x, 5));
    // The levels keep 28 from the ranks above and below.
    assert!(close(level(&edges[0]) - nodes[0].center.y - 20.0, 28.0, 5));
    assert!(close(gap, 2.0 * 28.0 + 2.0 * 12.0, 5));
}

#[test]
fn compound_draws_the_edges_of_a_gap_as_plain_curves_in_a_taller_gap_where_they_cross_each_other_more_than_they_run_alongside()
 {
    // Four nodes above four, each with an edge to every one below.
    let mut nodes: Vec<CNode> = (0..8)
        .map(|_| CNode::new(Size::new(80.0, 40.0), None))
        .collect();
    seq(&mut nodes);
    spans(&mut nodes, 56.0);
    let mut edges: Vec<CEdge> = Vec::new();
    for i in 0..4 {
        for j in 4..8 {
            edges.push(link(i, j));
        }
    }
    compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &OPTIONS);
    // No edge turns onto a level: each runs from one point to the other.
    for e in &edges {
        assert_eq!(e.route.len(), 2);
    }
    // The gap is half as tall as the widest edge runs across, and no more
    // than three times the usual.
    let widest = edges
        .iter()
        .map(|e| (e.route[0].at.x - e.route[1].at.x).abs())
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(widest > 6.0 * OPTIONS.rank_sep);
    assert!(close(
        nodes[4].center.y - nodes[0].center.y - 40.0,
        3.0 * OPTIONS.rank_sep,
        5
    ));
}

#[test]
fn compound_sets_a_rank_of_labels_half_an_arrowhead_nearer_the_tails_of_its_edges_and_holds_a_bend_beside_them_for_half_their_height()
 {
    let lay = |head_room: f64| -> Vec<CEdge> {
        let mut nodes = vec![
            CNode::new(Size::new(60.0, 40.0), None),
            CNode::new(Size::new(60.0, 40.0), None),
        ];
        seq(&mut nodes);
        spans(&mut nodes, 36.0);
        let mut edges = vec![edge(0, 1, 40.0, 20.0), edge(0, 1, 50.0, 20.0), link(0, 1)];
        let options = LayeredOptions {
            head_room: Some(head_room),
            ..OPTIONS
        };
        compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &options);
        edges
    };
    // Without arrowheads the labels are midway between the two nodes.
    let plain = lay(0.0);
    assert!(close(plain[0].label_at.y, (40.0 + 108.0) / 2.0, 5));
    // With them, the labels are midway between the node above and the backs
    // of the arrowheads.
    let edges = lay(8.0);
    let (first, second, bare) = (&edges[0], &edges[1], &edges[2]);
    assert!(close(first.label_at.y, (40.0 + 108.0 - 8.0) / 2.0, 5));
    assert!(close(second.label_at.y, first.label_at.y, 5));
    // A labelled edge runs straight for the height of its label, and the bare
    // one for half of that.
    assert!(close(first.route[2].at.y - first.route[1].at.y, 20.0, 5));
    assert!(close(bare.route[2].at.y - bare.route[1].at.y, 10.0, 5));
    assert!(close(
        (bare.route[1].at.y + bare.route[2].at.y) / 2.0,
        first.label_at.y,
        5
    ));
}

#[test]
fn compound_turns_an_edge_that_runs_far_across_at_a_level_of_its_own_where_a_shorter_one_would_run_close_beside_it()
 {
    // A node above a row of three, and an edge back up to it from below the
    // row.
    let mut nodes: Vec<CNode> = (0..5)
        .map(|_| CNode::new(Size::new(80.0, 40.0), None))
        .collect();
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
    compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &OPTIONS);
    // The edge back up passes the row on one side, then runs across to the
    // top node and up into it.
    let back = &edges[6].route;
    let n = back.len();
    let level = back[n - 2].at.y;
    assert!(close(back[n - 3].at.y, level, 5));
    assert!(close(back[n - 2].at.x, back[n - 1].at.x, 5));
    assert!((back[n - 3].at.x - back[n - 2].at.x).abs() > 2.0 * OPTIONS.rank_sep);
    // The edge down to the node on that side leaves next to where it arrives.
    // It runs straight down as far as that level and curves below it.
    let down = &edges[2].route;
    assert!((down[0].at.x - back[n - 1].at.x).abs() < 20.0);
    assert_eq!(down.len(), 3);
    assert!(close(down[1].at.x, down[0].at.x, 5));
    assert!(down[1].at.y > level - 0.01);
}

#[test]
fn compound_moves_an_end_along_its_side_to_let_its_edge_run_straight_and_no_closer_to_its_neighbour_than_a_pitch()
 {
    // A wide node above four narrow ones.
    let mut nodes = vec![
        CNode::new(Size::new(400.0, 40.0), None),
        CNode::new(Size::new(40.0, 40.0), None),
        CNode::new(Size::new(40.0, 40.0), None),
        CNode::new(Size::new(40.0, 40.0), None),
        CNode::new(Size::new(40.0, 40.0), None),
    ];
    seq(&mut nodes);
    for node in nodes.iter_mut() {
        node.span = node.size.w - 24.0;
    }
    let mut edges = vec![link(0, 1), link(0, 2), link(0, 3), link(0, 4)];
    compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &OPTIONS);
    // Each child is under the wide node, so each edge runs straight down to
    // it.
    for e in &edges {
        assert!(close(e.route[0].at.x, e.route[e.route.len() - 1].at.x, 5));
    }
    let mut two_way = vec![
        CNode::new(Size::new(200.0, 40.0), None),
        CNode::new(Size::new(40.0, 40.0), None),
    ];
    seq(&mut two_way);
    for node in two_way.iter_mut() {
        node.span = node.size.w - 24.0;
    }
    let mut pair = vec![link(0, 1), link(1, 0)];
    compound_layout(&mut two_way, &mut pair, Direction::TopToBottom, &OPTIONS);
    assert!(close(
        (pair[0].route[0].at.x - pair[1].route[pair[1].route.len() - 1].at.x).abs(),
        16.0,
        5
    ));
}

#[test]
fn compound_moves_the_edges_that_leave_side_by_side_closer_together_before_it_lets_them_share_a_place()
 {
    let mut nodes = vec![CNode::new(Size::new(60.0, 40.0), None)];
    let mut edges: Vec<CEdge> = Vec::new();
    for i in 1..=7 {
        nodes.push(CNode::new(Size::new(60.0, 40.0), None));
        edges.push(link(0, i));
    }
    seq(&mut nodes);
    nodes[0].span = 36.0;
    compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &OPTIONS);
    let mut starts: Vec<f64> = edges.iter().map(|e| e.route[0].at.x).collect();
    starts.sort_by(f64::total_cmp);
    // Six gaps of half a pitch each across the 36 there is.
    for i in 1..starts.len() {
        assert!(close(starts[i] - starts[i - 1], 6.0, 5));
    }
}

#[test]
fn compound_draws_an_edge_as_one_curve_across_a_rank_that_only_a_twin_of_another_edge_put_there() {
    let lay = |twin: bool| -> Vec<CEdge> {
        let mut nodes: Vec<CNode> = (0..4)
            .map(|_| CNode::new(Size::new(60.0, 40.0), None))
            .collect();
        seq(&mut nodes);
        spans(&mut nodes, 36.0);
        let mut edges = vec![link(0, 1), link(0, 2), link(0, 3)];
        if twin {
            edges.push(link(0, 3));
        }
        compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &OPTIONS);
        edges
    };
    let plain = lay(false);
    let twinned = lay(true);
    // The twins make every edge take a rank in the middle, but none of them
    // needs a bend there.
    for e in &twinned[..2] {
        assert_eq!(e.route.len(), 2);
    }
    assert!(close(twinned[0].route[0].at.y, plain[0].route[0].at.y, 5));
    // The gap is tall enough for the twins to turn at two levels, 12 apart
    // and 28 from the ranks.
    assert!(close(
        twinned[0].route[1].at.y,
        plain[0].route[1].at.y + 2.0 * 28.0 + 12.0 - OPTIONS.rank_sep,
        5
    ));
    // The twins leave and arrive at places of their own.
    let (one, other) = (&twinned[2].route, &twinned[3].route);
    assert!((one[0].at.x - other[0].at.x).abs() >= 8.0);
    assert!((one[one.len() - 1].at.x - other[other.len() - 1].at.x).abs() >= 8.0);
    // They would run close beside each other, so one turns straight away and
    // the other further down.
    let turns_at = |route: &[RoutePoint]| {
        if (route[1].at.x - route[0].at.x).abs() < 0.01 {
            route[1].at.y
        } else {
            route[0].at.y
        }
    };
    assert!((turns_at(one) - turns_at(other)).abs() >= 10.0);
}

#[test]
fn compound_keeps_twin_edges_apart_where_their_ends_cannot_be() {
    let mut nodes = vec![
        CNode::new(Size::new(60.0, 40.0), None),
        CNode::new(Size::new(60.0, 40.0), None),
    ];
    seq(&mut nodes);
    let mut edges = vec![link(0, 1), link(0, 1)];
    compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &OPTIONS);
    let (a, b) = (&edges[0].route, &edges[1].route);
    assert_eq!(a.len(), 3);
    assert_eq!(b.len(), 3);
    assert!(close(a[0].at.x, b[0].at.x, 5));
    assert!(close((a[1].at.x - b[1].at.x).abs(), OPTIONS.edge_sep, 5));
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
            let mut node = CNode::new(Size::new(w, h), None);
            node.seq = i as f64;
            node.span = node.size.w - 24.0;
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
        compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &options);
        let at = format!("seed {seed}");
        let mut runs: Vec<Run> = Vec::new();
        for (ei, e) in edges.iter().enumerate() {
            let r = &e.route;
            assert!(
                on_border(&nodes[e.src], r[0].at),
                "{at} edge {ei} starts on its source"
            );
            assert!(
                on_border(&nodes[e.dst], r[r.len() - 1].at),
                "{at} edge {ei} ends on its target"
            );
            for k in 1..r.len() {
                let (p, q) = (r[k - 1].at, r[k].at);
                // Two ends less than half a unit out of line are joined
                // directly.
                let across = (q.y - p.y).abs() < 0.01;
                let down = (q.x - p.x).abs() < 0.5;
                assert!(across || down, "{at} edge {ei} turns at right angles");
                if !across || down {
                    continue;
                }
                // The route runs upwards when the edge does, so the higher
                // end is the one it comes down to.
                let before = if k >= 2 { r[k - 2].at.y } else { p.y };
                let from_first =
                    before < p.y - 0.01 || (k + 1 < r.len() && r[k + 1].at.y > q.y + 0.01);
                runs.push(Run {
                    y: q.y,
                    lo: q.x.min(p.x),
                    hi: q.x.max(p.x),
                    from: if from_first { p.x } else { q.x },
                    to: if from_first { q.x } else { p.x },
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
                let inside = (run.y - node.center.y).abs() < node.size.h / 2.0 - 0.5
                    && run.lo < node.center.x + node.size.w / 2.0 - 0.5
                    && run.hi > node.center.x - node.size.w / 2.0 + 0.5;
                assert!(!inside, "{at} a run crosses a node");
            }
        }
    }
}

#[test]
fn compound_nests_the_tracks_of_a_fan_so_that_its_edges_do_not_cross() {
    let mut nodes = vec![
        CNode::new(Size::new(120.0, 40.0), None),
        CNode::new(Size::new(60.0, 40.0), None),
        CNode::new(Size::new(60.0, 40.0), None),
        CNode::new(Size::new(60.0, 40.0), None),
        CNode::new(Size::new(60.0, 40.0), None),
    ];
    seq(&mut nodes);
    nodes[0].span = 96.0;
    let mut edges = vec![link(0, 1), link(0, 2), link(0, 3), link(0, 4)];
    let options = LayeredOptions {
        tracks: true,
        ..OPTIONS
    };
    compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &options);
    let across_y = |e: &CEdge| e.route[1].at.y;
    let mut left: Vec<&CEdge> = edges
        .iter()
        .filter(|e| nodes[e.dst].center.x < nodes[0].center.x)
        .collect();
    left.sort_by(|a, b| nodes[a.dst].center.x.total_cmp(&nodes[b.dst].center.x));
    let mut right: Vec<&CEdge> = edges
        .iter()
        .filter(|e| nodes[e.dst].center.x > nodes[0].center.x)
        .collect();
    right.sort_by(|a, b| nodes[b.dst].center.x.total_cmp(&nodes[a.dst].center.x));
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
    let mut a = CNode::new(Size::default(), Some(1));
    let mut b = CNode::new(Size::default(), Some(0));
    a.is_group = true;
    b.is_group = true;
    let leaf = CNode::new(Size::new(40.0, 40.0), Some(0));
    let mut nodes = vec![a, b, leaf, CNode::new(Size::new(40.0, 40.0), None)];
    let mut edges = vec![link(2, 3)];
    let size = compound_layout(&mut nodes, &mut edges, Direction::TopToBottom, &OPTIONS);
    assert!(size.w.is_finite());
    assert!(edges[0].route.len() >= 2);
}

fn group(parent: Option<usize>) -> CNode {
    let mut node = CNode::new(Size::default(), parent);
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
    dir: Direction,
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
        nodes.push(CNode::new(Size::new(w, h), None));
        leaves.push(i);
    }
    for _ in 0..lane_count {
        lanes.push(nodes.len());
        nodes.push(group(None));
    }
    // The bound is drawn again before every turn, as Pele's loop does.
    let mut k = 0;
    while k < rnd.int(4) {
        let lane = lanes[rnd.int(lane_count)];
        inner.push(nodes.len());
        inner_lane.push(lane);
        nodes.push(group(Some(lane)));
        k += 1;
    }
    for &i in &leaves {
        if !inner.is_empty() && rnd.next() < 0.25 {
            let g = rnd.int(inner.len());
            nodes[i].parent = Some(inner[g]);
            lane_of[i] = inner_lane[g];
        } else {
            lane_of[i] = lanes[rnd.int(lane_count)];
            nodes[i].parent = Some(lane_of[i]);
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
    let result = lane_layout(&mut [], &mut [], Direction::TopToBottom, &OPTIONS, None);
    assert_eq!((result.w, result.h), (0.0, 0.0));
}

#[test]
fn lanes_sit_side_by_side_across_the_flow_in_the_order_written() {
    for dir in DIRS {
        let mut nodes = vec![
            CNode::new(Size::new(60.0, 30.0), None),
            CNode::new(Size::new(60.0, 30.0), None),
            group(None),
            group(None),
        ];
        nodes[0].parent = Some(3);
        nodes[1].parent = Some(2);
        let result = lane_layout(&mut nodes, &mut [link(0, 1)], dir, &OPTIONS, None);
        let (first, second) = (&nodes[3], &nodes[2]);
        if dir.is_vertical() {
            assert!(first.center.x < second.center.x, "{dir:?}");
            assert_eq!(first.size.h, result.h, "{dir:?}");
            assert!(close(first.size.w + second.size.w, result.w, 6), "{dir:?}");
        } else {
            assert!(first.center.y < second.center.y, "{dir:?}");
            assert_eq!(first.size.w, result.w, "{dir:?}");
            assert!(close(first.size.h + second.size.h, result.h, 6), "{dir:?}");
        }
    }
}

#[test]
fn lanes_run_the_flow_in_the_direction_asked_for() {
    let at = |dir: Direction| -> Vec<CNode> {
        let mut nodes = vec![
            CNode::new(Size::new(60.0, 30.0), Some(2)),
            CNode::new(Size::new(60.0, 30.0), Some(2)),
            group(None),
        ];
        lane_layout(&mut nodes, &mut [link(0, 1)], dir, &OPTIONS, None);
        nodes
    };
    assert!(at(Direction::TopToBottom)[0].center.y < at(Direction::TopToBottom)[1].center.y);
    assert!(at(Direction::BottomToTop)[0].center.y > at(Direction::BottomToTop)[1].center.y);
    assert!(at(Direction::LeftToRight)[0].center.x < at(Direction::LeftToRight)[1].center.x);
    assert!(at(Direction::RightToLeft)[0].center.x > at(Direction::RightToLeft)[1].center.x);
}

#[test]
fn lanes_keep_a_chain_in_one_lane_on_a_straight_line() {
    let mut nodes = vec![
        CNode::new(Size::new(50.0, 30.0), Some(3)),
        CNode::new(Size::new(120.0, 30.0), Some(3)),
        CNode::new(Size::new(70.0, 30.0), Some(3)),
        group(None),
    ];
    let mut edges = vec![link(0, 1), link(1, 2)];
    lane_layout(
        &mut nodes,
        &mut edges,
        Direction::TopToBottom,
        &OPTIONS,
        None,
    );
    assert!(close(nodes[0].center.x, nodes[1].center.x, 6));
    assert!(close(nodes[1].center.x, nodes[2].center.x, 6));
}

#[test]
fn lanes_separate_an_approval_decisions_incoming_accepted_and_returned_paths() {
    for dir in DIRS {
        let mut nodes = vec![
            CNode::new(Size::new(60.0, 40.0), Some(8)),
            CNode::new(Size::new(80.0, 40.0), Some(7)),
            CNode::new(Size::new(70.0, 70.0), Some(7)),
            CNode::new(Size::new(80.0, 40.0), Some(6)),
            CNode::new(Size::new(80.0, 40.0), Some(6)),
            CNode::new(Size::new(60.0, 40.0), Some(8)),
            group(None),
            group(None),
            group(None),
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
        let incoming = end_side(&edges[1].route, edges[1].route.len() - 1, &nodes[2]);
        let accepted = end_side(&edges[2].route, 0, &nodes[2]);
        let returned = end_side(&edges[5].route, 0, &nodes[2]);
        let sides: HashSet<Option<Side>> = [incoming, accepted, returned].into_iter().collect();
        assert_eq!(sides.len(), 3, "{dir:?}");
        assert!(!sides.contains(&None), "{dir:?}");
        let back = &edges[5];
        let r = &back.route;
        let mut corners = 0;
        for p in 2..r.len() {
            if (r[p].at.x == r[p - 1].at.x) != (r[p - 1].at.x == r[p - 2].at.x) {
                corners += 1;
            }
        }
        assert_eq!(corners, 1, "{dir:?}");
        for p in 1..r.len() {
            assert!(
                r[p].at.x == r[p - 1].at.x || r[p].at.y == r[p - 1].at.y,
                "{dir:?}"
            );
        }
        for node in &nodes[..6] {
            let [l, t, r, b] = boxed(node);
            let clear = back.label_at.x + back.label.w / 2.0 <= l
                || back.label_at.x - back.label.w / 2.0 >= r
                || back.label_at.y + back.label.h / 2.0 <= t
                || back.label_at.y - back.label.h / 2.0 >= b;
            assert!(clear, "{dir:?}");
        }
    }
}

#[test]
fn lanes_keep_a_handoff_into_a_step_apart_from_the_handoff_out_of_it() {
    for dir in DIRS {
        let mut nodes = vec![
            CNode::new(Size::new(110.0, 38.0), Some(5)),
            CNode::new(Size::new(110.0, 38.0), Some(5)),
            CNode::new(Size::new(100.0, 38.0), Some(4)),
            CNode::new(Size::new(110.0, 38.0), Some(4)),
            group(None),
            group(None),
        ];
        let mut edges = vec![link(2, 3), link(0, 3), link(3, 1)];
        lane_layout(&mut nodes, &mut edges, dir, &OPTIONS, None);
        let (into, out) = (&edges[1].route, &edges[2].route);
        assert_ne!(
            end_side(into, into.len() - 1, &nodes[3]),
            end_side(out, 0, &nodes[3]),
            "{dir:?}"
        );
        for p in 1..into.len() {
            for q in 1..out.len() {
                let (a0, a1) = (into[p - 1].at, into[p].at);
                let (b0, b1) = (out[q - 1].at, out[q].at);
                let upright = a1.x == a0.x && b1.x == b0.x && a1.x == b1.x;
                let level = a1.y == a0.y && b1.y == b0.y && a1.y == b1.y;
                let k = if upright { Axis::Y } else { Axis::X };
                let shared = a1
                    .along(k)
                    .max(a0.along(k))
                    .min(b1.along(k).max(b0.along(k)))
                    - a1.along(k)
                        .min(a0.along(k))
                        .max(b1.along(k).min(b0.along(k)));
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
            CNode::new(Size::new(140.0, 44.0), Some(8)),
            CNode::new(Size::new(140.0, 44.0), Some(8)),
            CNode::new(Size::new(140.0, 44.0), Some(7)),
            CNode::new(Size::new(120.0, 76.0), Some(7)),
            CNode::new(Size::new(140.0, 44.0), Some(6)),
            CNode::new(Size::new(140.0, 44.0), Some(6)),
            group(None),
            group(None),
            group(None),
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
        let sideways = !dir.is_vertical();
        let across = |c: &CNode| if sideways { c.center.y } else { c.center.x };
        let along = |c: &CNode| if sideways { c.center.x } else { c.center.y };
        let depth = |c: &CNode| if sideways { c.size.w } else { c.size.h };
        assert!(close(across(&nodes[0]), across(&nodes[1]), 6), "{dir:?}");
        assert!(close(across(&nodes[4]), across(&nodes[5]), 6), "{dir:?}");
        let back = &edges[5].route;
        let sides: HashSet<Option<Side>> = [
            end_side(&edges[2].route, edges[2].route.len() - 1, &nodes[3]),
            end_side(&edges[3].route, 0, &nodes[3]),
            end_side(back, 0, &nodes[3]),
        ]
        .into_iter()
        .collect();
        assert_eq!(sides.len(), 3, "{dir:?}");
        assert_ne!(
            end_side(back, back.len() - 1, &nodes[0]),
            end_side(&edges[0].route, 0, &nodes[0]),
            "{dir:?}"
        );
        for p in 1..back.len() {
            let (from, to) = (back[p - 1].at, back[p].at);
            assert!(from.x == to.x || from.y == to.y, "{dir:?}");
            for (i, node) in nodes[..6].iter().enumerate() {
                if i == 0 || i == 3 {
                    continue;
                }
                let [l, t, r, b] = boxed(node);
                assert!(
                    to.x.max(from.x) <= l
                        || to.x.min(from.x) >= r
                        || to.y.max(from.y) <= t
                        || to.y.min(from.y) >= b,
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
            CNode::new(Size::new(80.0, 40.0), Some(5)),
            CNode::new(Size::new(80.0, 40.0), Some(5)),
            CNode::new(Size::new(80.0, 40.0), Some(5)),
            CNode::new(Size::new(80.0, 40.0), Some(4)),
            group(None),
            group(None),
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
        let sideways = !dir.is_vertical();
        let gap = |a: usize, b: usize| {
            (if sideways {
                nodes[b].center.x - nodes[a].center.x
            } else {
                nodes[b].center.y - nodes[a].center.y
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
        CNode::new(Size::new(110.0, 38.0), Some(7)),
        CNode::new(Size::new(110.0, 38.0), Some(6)),
        CNode::new(Size::new(110.0, 38.0), Some(6)),
        CNode::new(Size::new(110.0, 38.0), Some(6)),
        CNode::new(Size::new(110.0, 38.0), Some(6)),
        CNode::new(Size::new(110.0, 38.0), Some(6)),
        group(None),
        group(None),
    ];
    let mut edges = vec![
        link(0, 1),
        link(1, 2),
        link(2, 3),
        link(3, 4),
        link(4, 5),
        edge(6, 5, 400.0, 200.0),
    ];
    lane_layout(
        &mut nodes,
        &mut edges,
        Direction::TopToBottom,
        &OPTIONS,
        None,
    );
    let handoff = &edges[0].route;
    assert!(handoff.len() > 2);
    for p in 1..handoff.len() {
        let (from, to) = (handoff[p - 1].at, handoff[p].at);
        assert!(from.x == to.x || from.y == to.y);
    }
    for point in handoff {
        assert!((point.at.y - edges[5].label_at.y).abs() > 100.0);
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
        assert!(result.w.is_finite() && result.h.is_finite(), "{at}");
        for c in &nodes {
            assert!(
                c.center.x.is_finite()
                    && c.center.y.is_finite()
                    && c.size.w.is_finite()
                    && c.size.h.is_finite(),
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
            if !leaves.iter().any(|&i| nodes[i].parent == Some(g)) {
                continue;
            }
            let [l, t, r, b] = boxed(&nodes[g]);
            let [ll, lt, lr, lb] = boxed(&nodes[inner_lane[k]]);
            assert!(
                l >= ll - 0.01 && r <= lr + 0.01 && t >= lt - 0.01 && b <= lb + 0.01,
                "{at}: group {g} in its lane"
            );
            for &i in &leaves {
                if nodes[i].parent != Some(g) {
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
                l >= -0.01 && t >= -0.01 && r <= result.w + 0.01 && b <= result.h + 0.01,
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
            assert!(e.route.len() >= 2, "{at}: route");
            for point in &e.route {
                let p = point.at;
                assert!(p.x.is_finite() && p.y.is_finite(), "{at}: route value");
                let inside = p.x >= -0.01
                    && p.x <= result.w + 0.01
                    && p.y >= -0.01
                    && p.y <= result.h + 0.01;
                assert!(inside, "{at}: route inside the lanes");
            }
            assert!(
                e.label_at.x.is_finite() && e.label_at.y.is_finite(),
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
            .map(|c| [c.center.x, c.center.y, c.size.w, c.size.h])
            .collect::<Vec<_>>()
    };
    assert_eq!(shape(&a.nodes), shape(&b.nodes));
    let routes = |edges: &[CEdge]| edges.iter().map(|e| e.route.clone()).collect::<Vec<_>>();
    assert_eq!(routes(&a.edges), routes(&b.edges));
}
