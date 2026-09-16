use canvas::{
    Canvas, Change, change, contain,
    drag::{self, Drag, DragHandler, Phase},
    layout,
    mindmap::{self, GAP_X},
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

/// root → a, b; a → a1
fn tree() -> Canvas {
    let mut canvas = Canvas {
        nodes: ["root", "a", "b", "a1"].map(node).into(),
        edges: vec![
            Edge::new("e1", "root", "a"),
            Edge::new("e2", "root", "b"),
            Edge::new("e3", "a", "a1"),
        ],
        ..Canvas::default()
    };
    mindmap::layout(&mut canvas);
    canvas
}

/// Drop `id` over `over`, 30 right and 40 down of the origin.
fn drop(handler: DragHandler, canvas: &mut Canvas, id: &str, over: Option<&str>) -> Vec<Change> {
    let gesture = Drag {
        id,
        with: &[],
        contents: &[],
        origin: (0, 0),
        delta: (30, 40),
        over,
        phase: Phase::Drop,
    };
    let changes = handler(canvas, &gesture);
    for change in &changes {
        change::apply(canvas, change);
    }
    changes
}

fn at(canvas: &Canvas, id: &str) -> (i64, i64) {
    let node = canvas.node(id).unwrap();
    (node.x, node.y)
}

#[test]
fn pin_keeps_the_drop() {
    let mut canvas = tree();
    drop(drag::pin, &mut canvas, "a", None);
    assert_eq!(at(&canvas, "a"), (30, 40));
    assert!(mindmap::is_pinned(canvas.node("a").unwrap()));
}

#[test]
fn reparent_moves_the_edge_and_the_branch_with_it() {
    let mut canvas = tree();
    drop(drag::reparent, &mut canvas, "a", Some("b"));
    assert_eq!(mindmap::parent(&canvas, "a"), Some("b"));
    assert_eq!(mindmap::parent(&canvas, "a1"), Some("a"));
}

#[test]
fn reparent_refuses_its_own_branch() {
    let mut canvas = tree();
    drop(drag::reparent, &mut canvas, "a", Some("a1"));
    assert_eq!(mindmap::parent(&canvas, "a"), Some("root"));
}

#[test]
fn reparent_over_nothing_changes_nothing() {
    let mut canvas = tree();
    assert!(drop(drag::reparent, &mut canvas, "a", None).is_empty());
}

#[test]
fn detach_cuts_the_edges_in() {
    let mut canvas = tree();
    drop(drag::detach, &mut canvas, "a", Some("b"));
    assert_eq!(mindmap::parent(&canvas, "a"), None);
    assert_eq!(mindmap::parent(&canvas, "a1"), Some("a"));
    mindmap::layout(&mut canvas);
    assert_eq!(at(&canvas, "a"), (30, 40));
}

#[test]
fn a_move_answers_what_the_drop_would_do() {
    let canvas = tree();
    let moving = |over| Drag {
        id: "a",
        with: &[],
        contents: &[],
        origin: (0, 0),
        delta: (5, 5),
        over,
        phase: Phase::Move,
    };
    let intents = |changes: Vec<Change>| -> Vec<Change> {
        changes
            .into_iter()
            .filter(|change| !matches!(change, Change::MoveNodes { .. }))
            .collect()
    };
    let cut = || Change::RemoveEdges {
        ids: vec!["e1".into()],
    };
    let hang = intents(drag::reparent(&canvas, &moving(Some("b"))));
    assert!(
        matches!(&hang[..], [removed, Change::AddEdge { edge, .. }]
            if *removed == cut() && (edge.from_node.as_str(), edge.to_node.as_str()) == ("b", "a")),
        "{hang:?}"
    );
    assert!(intents(drag::reparent(&canvas, &moving(None))).is_empty());
    assert_eq!(intents(drag::detach(&canvas, &moving(None))), [cut()]);
    assert!(intents(drag::pin(&canvas, &moving(None))).is_empty());
}

#[test]
fn a_drop_skips_what_the_layout_carries() {
    let canvas = tree();
    let inside = |id| {
        let (x, y) = at(&canvas, id);
        (x + 10, y + 10)
    };
    // What a tree layout carries with `a`: its branch.
    let held = (layout::MINDMAP.reach)(&canvas, "a");
    let any = |_: &canvas::model::Node| true;
    let bare = |_: &canvas::model::Node| false;
    assert_eq!(
        contain::topmost(&canvas, inside("a1"), &held, any, bare),
        None
    );
    assert_eq!(
        contain::topmost(&canvas, inside("b"), &held, any, bare),
        Some("b")
    );
}

#[test]
fn a_held_node_stays_and_its_branch_follows() {
    let mut canvas = tree();
    let a = canvas.node_mut("a").unwrap();
    (a.x, a.y) = (500, 500);
    mindmap::layout_holding(&mut canvas, Some("a"));
    assert_eq!(at(&canvas, "a"), (500, 500));
    assert_eq!(at(&canvas, "a1"), (500 + 100 + GAP_X, 500));
}
