use canvas::{
    Canvas, Change, change,
    layout::{self, Arrow},
    mindmap::{self, Flow, GAP_X, GAP_Y},
    model::{Edge, Node, TEXT},
};

fn node(id: &str) -> Node {
    Node {
        id: id.into(),
        kind: TEXT.into(),
        width: 100,
        height: 40,
        ..Node::default()
    }
}

/// root → a, b, c
fn fan(flow: Flow) -> Canvas {
    let mut canvas = Canvas {
        nodes: ["root", "a", "b", "c"].map(node).into(),
        edges: vec![
            Edge::new("e1", "root", "a"),
            Edge::new("e2", "root", "b"),
            Edge::new("e3", "root", "c"),
        ],
        ..Canvas::default()
    };
    let moves = mindmap::arrange(&canvas, None, flow);
    change::apply(&mut canvas, &Change::MoveNodes { moves });
    canvas
}

fn at(canvas: &Canvas, id: &str) -> (i64, i64) {
    let node = canvas.node(id).unwrap();
    (node.x, node.y)
}

#[test]
fn down_puts_children_in_a_row_below() {
    let canvas = fan(Flow::Down);
    let [a, b, c] = ["a", "b", "c"].map(|id| at(&canvas, id));
    assert!([a, b, c].iter().all(|(_, y)| *y == 40 + GAP_X));
    assert_eq!(b.0 - a.0, 100 + GAP_Y);
    // Centred: the row's middle is the root's middle.
    assert_eq!((a.0 + c.0 + 100) / 2, 50);
}

#[test]
fn balanced_splits_a_root_across_its_sides() {
    let canvas = fan(Flow::Both);
    assert_eq!(at(&canvas, "a").0, 100 + GAP_X);
    assert_eq!(at(&canvas, "b").0, -GAP_X - 100);
    assert_eq!(at(&canvas, "c").0, 100 + GAP_X);
}

#[test]
fn arrows_walk_the_way_the_tree_grows() {
    let down = fan(Flow::Down);
    let walk = |id, arrow| mindmap::walk(&down, id, Flow::Down, arrow);
    assert_eq!(walk("root", Arrow::Down).as_deref(), Some("a"));
    assert_eq!(walk("a", Arrow::Right).as_deref(), Some("b"));
    assert_eq!(walk("b", Arrow::Up).as_deref(), Some("root"));
    assert_eq!(walk("a", Arrow::Left), None);

    let both = fan(Flow::Both);
    let walk = |id, arrow| mindmap::walk(&both, id, Flow::Both, arrow);
    assert_eq!(walk("root", Arrow::Left).as_deref(), Some("b"));
    assert_eq!(walk("root", Arrow::Right).as_deref(), Some("a"));
    assert_eq!(walk("b", Arrow::Right).as_deref(), Some("root"));
    assert_eq!(walk("a", Arrow::Down).as_deref(), Some("c"));
}

#[test]
fn nearest_looks_along_the_arrow() {
    let place = |id, x, y| Node { x, y, ..node(id) };
    let canvas = Canvas {
        nodes: vec![
            place("o", 0, 0),
            place("right", 300, 60),
            place("far", 700, 0),
            place("below", 20, 300),
        ],
        ..Canvas::default()
    };
    let nearest = |arrow| layout::nearest(&canvas, "o", arrow);
    assert_eq!(nearest(Arrow::Right), Some("right"));
    assert_eq!(nearest(Arrow::Down), Some("below"));
    assert_eq!(nearest(Arrow::Left), None);
}
