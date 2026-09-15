use canvas::{
    Canvas,
    mindmap::{self, GAP_X, GAP_Y},
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

/// root → a, b, c; b → b1
fn tree() -> Canvas {
    Canvas {
        nodes: ["root", "a", "b", "c", "b1"].map(node).into(),
        edges: vec![
            Edge::new("e1", "root", "a"),
            Edge::new("e2", "root", "b"),
            Edge::new("e3", "root", "c"),
            Edge::new("e4", "b", "b1"),
        ],
        ..Canvas::default()
    }
}

fn at(canvas: &Canvas, id: &str) -> (i64, i64) {
    let node = canvas.node(id).unwrap();
    (node.x, node.y)
}

#[test]
fn a_pinned_node_stays_and_its_branch_follows() {
    let mut canvas = tree();
    mindmap::layout(&mut canvas);
    mindmap::pin(&mut canvas, "b", 500, 400).unwrap();
    mindmap::layout(&mut canvas);
    assert_eq!(at(&canvas, "b"), (500, 400));
    assert_eq!(at(&canvas, "b1"), (500 + 100 + GAP_X, 400));
    // The column closes over the gap it left.
    assert_eq!(at(&canvas, "c").1 - at(&canvas, "a").1, 40 + GAP_Y);
}

#[test]
fn a_pinned_node_rides_along_with_its_ancestor() {
    let mut canvas = tree();
    mindmap::layout(&mut canvas);
    mindmap::pin(&mut canvas, "b1", 700, 300).unwrap();
    let (bx, by) = at(&canvas, "b");
    canvas::change::apply(
        &mut canvas,
        &canvas::Change::Move {
            id: "b".into(),
            to: (bx + 10, by + 20),
            pin: true,
        },
    );
    assert_eq!(at(&canvas, "b1"), (710, 320));
    // Moving it back carries it back.
    canvas::change::apply(
        &mut canvas,
        &canvas::Change::Move {
            id: "b".into(),
            to: (bx, by),
            pin: false,
        },
    );
    assert_eq!(at(&canvas, "b1"), (700, 300));
}

#[test]
fn a_pin_survives_a_save() {
    let mut canvas = tree();
    mindmap::pin(&mut canvas, "a", 7, 9).unwrap();
    let saved = Canvas::parse(&canvas.to_json()).unwrap();
    assert!(mindmap::is_pinned(saved.node("a").unwrap()));
    assert!(!mindmap::is_pinned(saved.node("c").unwrap()));
}
