//! Any node holds others: by name, or by the box of a node whose kind holds.

use canvas::{
    Canvas, change, clip, contain,
    model::{GROUP, Node, TEXT},
};

fn node(id: &str, kind: &str, (x, y, width, height): (i64, i64, i64, i64)) -> Node {
    Node {
        id: id.into(),
        kind: kind.into(),
        x,
        y,
        width,
        height,
        ..Node::default()
    }
}

fn naming(mut node: Node, container: &str) -> Node {
    node.extra
        .insert(contain::CONTAINER.into(), container.into());
    node
}

fn groups(node: &Node) -> bool {
    node.kind == GROUP
}

/// g holds a by its box; b, far off, names a.
fn nested() -> Canvas {
    Canvas {
        nodes: vec![
            node("g", GROUP, (0, 0, 400, 300)),
            node("a", TEXT, (20, 20, 100, 40)),
            naming(node("b", TEXT, (600, 0, 100, 40)), "a"),
        ],
        ..Canvas::default()
    }
}

#[test]
fn a_named_container_holds_wherever_its_node_sits() {
    let canvas = nested();
    assert_eq!(
        contain::with_contents(&canvas, &["g".into()], groups),
        ["g", "a", "b"]
    );
    let depths = contain::depths(&canvas, groups);
    assert_eq!((depths["g"], depths["a"], depths["b"]), (0, 1, 2));
}

#[test]
fn the_smallest_holder_around_takes_a_node() {
    let canvas = Canvas {
        nodes: vec![
            node("outer", GROUP, (0, 0, 500, 500)),
            node("inner", GROUP, (10, 10, 200, 200)),
            node("x", TEXT, (20, 20, 50, 50)),
        ],
        ..Canvas::default()
    };
    let containers = contain::containers(&canvas, groups);
    assert_eq!((containers["x"], containers["inner"]), ("inner", "outer"));
    assert!(!containers.contains_key("outer"));
}

#[test]
fn carried_out_of_its_named_container_a_node_lets_it_go() {
    let mut canvas = Canvas {
        nodes: vec![
            node("box", TEXT, (0, 0, 300, 300)),
            naming(node("in", TEXT, (10, 10, 50, 50)), "box"),
        ],
        ..Canvas::default()
    };
    assert!(contain::loosen(&canvas, &["in".into()]).is_empty());
    canvas.node_mut("in").unwrap().x = 600;
    let loosened = contain::loosen(&canvas, &["in".into()]);
    change::apply_all(&mut canvas, &loosened);
    assert_eq!(contain::named(canvas.node("in").unwrap()), None);
}

#[test]
fn a_paste_names_its_container_afresh() {
    let canvas = nested();
    let both = clip::fragment(&canvas, &["a".into(), "b".into()]);
    let mut pasted = canvas.clone();
    change::apply_all(&mut pasted, &clip::paste(&canvas, &both, (0, 500), None));
    let (a, b) = (&pasted.nodes[3], &pasted.nodes[4]);
    assert_eq!(contain::named(b), Some(a.id.as_str()));

    let alone = clip::fragment(&canvas, &["b".into()]);
    let mut pasted = canvas.clone();
    change::apply_all(&mut pasted, &clip::paste(&canvas, &alone, (0, 500), None));
    assert_eq!(contain::named(pasted.nodes.last().unwrap()), None);
}
