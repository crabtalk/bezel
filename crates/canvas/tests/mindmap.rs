use canvas::{
    Canvas, Change, change,
    mindmap::{self, GAP_X, GAP_Y, Toward},
    model::{Edge, Node, TEXT},
};

fn node(id: &str, height: i64) -> Node {
    Node {
        id: id.into(),
        kind: TEXT.into(),
        width: 100,
        height,
        text: Some(id.into()),
        ..Node::default()
    }
}

/// root → a, b; a → a1
fn tree() -> Canvas {
    Canvas {
        nodes: vec![
            node("root", 40),
            node("a", 40),
            node("b", 40),
            node("a1", 40),
        ],
        edges: vec![
            Edge::new("e1", "root", "a"),
            Edge::new("e2", "root", "b"),
            Edge::new("e3", "a", "a1"),
        ],
        ..Canvas::default()
    }
}

fn at(canvas: &Canvas, id: &str) -> (i64, i64) {
    let node = canvas.node(id).unwrap();
    (node.x, node.y)
}

fn land(canvas: &mut Canvas, changes: Option<Vec<Change>>) -> String {
    let changes = changes.expect("a change");
    let id = change::added(&changes).expect("a node").to_owned();
    for change in &changes {
        change::apply(canvas, change);
    }
    id
}

#[test]
fn children_sit_in_a_column_centred_on_the_parent() {
    let mut canvas = tree();
    mindmap::layout(&mut canvas);
    assert_eq!(at(&canvas, "root"), (0, 0));
    let column = 100 + GAP_X;
    let (ax, ay) = at(&canvas, "a");
    let (bx, by) = at(&canvas, "b");
    assert_eq!((ax, bx), (column, column));
    assert_eq!(by - ay, 40 + GAP_Y);
    // Centred: the stack's middle is the root's middle.
    assert_eq!((ay + by + 40) / 2, 20);
    assert_eq!(at(&canvas, "a1"), (column * 2, ay));
}

#[test]
fn a_tall_subtree_pushes_its_siblings_apart() {
    let mut canvas = tree();
    canvas.nodes.push(node("a2", 40));
    canvas.edges.push(Edge::new("e4", "a", "a2"));
    mindmap::layout(&mut canvas);
    let (_, ay1) = at(&canvas, "a1");
    let (_, ay2) = at(&canvas, "a2");
    let (_, by) = at(&canvas, "b");
    assert_eq!(by, ay2 + 40 + GAP_Y);
    assert!(ay1 < ay2);
}

#[test]
fn a_cycle_does_not_loop() {
    let mut canvas = tree();
    canvas.edges.push(Edge::new("back", "a1", "a"));
    mindmap::layout(&mut canvas);
}

#[test]
fn changes_keep_order_and_ids_apart() {
    let mut canvas = tree();
    let change = mindmap::sibling(&canvas, "a", node("", 40));
    let new = land(&mut canvas, change);
    let order: Vec<&str> = mindmap::children(&canvas, "root").collect();
    assert_eq!(order, ["a", new.as_str(), "b"]);

    let change = mindmap::child(&canvas, "b", node("", 40));
    let child = land(&mut canvas, change);
    assert_eq!(mindmap::parent(&canvas, &child), Some("b"));
    assert!(mindmap::sibling(&canvas, "root", node("", 40)).is_none());
    assert!(canvas.edges.iter().all(|e| canvas.node(&e.id).is_none()));

    assert_eq!(
        mindmap::after_removal(&canvas, "a").as_deref(),
        Some(new.as_str())
    );
    assert_eq!(
        mindmap::after_removal(&canvas, "b").as_deref(),
        Some(new.as_str())
    );
    let remove = mindmap::remove(&canvas, "a");
    change::apply(&mut canvas, &remove);
    assert!(canvas.node("a1").is_none());
    assert!(canvas.edges.iter().all(|e| e.to_node != "a1"));
}

/// root → a → a1, and a cross link from b to a1.
fn crossed() -> Canvas {
    let mut canvas = tree();
    let mut link = Edge::new("x", "b", "a1");
    link.extra.insert(mindmap::TREE.into(), false.into());
    canvas.edges.insert(0, link);
    canvas
}

#[test]
fn a_cross_link_is_not_a_branch() {
    let canvas = crossed();
    assert_eq!(mindmap::parent(&canvas, "a1"), Some("a"));
    assert_eq!(mindmap::children(&canvas, "b").count(), 0);
    let Change::RemoveNodes { ids } = mindmap::remove(&canvas, "b") else {
        panic!("a removal");
    };
    assert_eq!(ids, ["b"]);
    let cut = Change::RemoveEdges {
        ids: vec!["e3".into()],
    };
    assert_eq!(mindmap::detach(&canvas, "a1"), Some(cut));
}

#[test]
fn removing_a_node_takes_every_edge_touching_it() {
    let mut canvas = crossed();
    change::apply(
        &mut canvas,
        &Change::RemoveNodes {
            ids: vec!["a1".into()],
        },
    );
    assert!(canvas.node("a").is_some());
    assert!(
        canvas
            .edges
            .iter()
            .all(|e| e.from_node != "a1" && e.to_node != "a1")
    );
}

#[test]
fn steps_walk_the_tree() {
    let canvas = tree();
    let step = |id, toward| mindmap::step(&canvas, id, toward);
    assert_eq!(step("root", Toward::FirstChild).as_deref(), Some("a"));
    assert_eq!(step("a", Toward::NextSibling).as_deref(), Some("b"));
    assert_eq!(step("a", Toward::PrevSibling), None);
    assert_eq!(step("a1", Toward::Parent).as_deref(), Some("a"));
}
