use crate::{
    change::Change,
    edge::{EdgeKind, EdgeKinds},
    edit::CanvasEditor,
    layout,
    model::{Canvas, Edge, LINK, Node},
    path,
    view::{Positions, edge_paths::EdgePaths},
};
use gpui::point;
use std::rc::Rc;

fn editor() -> CanvasEditor {
    CanvasEditor::new(
        Canvas {
            nodes: (0..4)
                .map(|index| Node {
                    id: index.to_string(),
                    kind: LINK.into(),
                    x: index * 300,
                    width: 100,
                    height: 100,
                    ..Node::default()
                })
                .collect(),
            edges: vec![
                Edge {
                    id: "a".into(),
                    kind: Some(crate::edge::CURVE.into()),
                    from_node: "0".into(),
                    to_node: "1".into(),
                    ..Edge::default()
                },
                Edge {
                    id: "b".into(),
                    kind: Some(crate::edge::CURVE.into()),
                    from_node: "2".into(),
                    to_node: "3".into(),
                    ..Edge::default()
                },
            ],
            ..Canvas::default()
        },
        layout::FREE,
    )
}

#[test]
fn paths_survive_camera_changes_and_only_changed_endpoints_rebuild() {
    let mut editor = editor();
    let mut paths = EdgePaths::default();
    let shown = Positions::new();
    paths.refresh(&editor, &shown);
    let a = paths.get("a").unwrap().clone();
    let b = paths.get("b").unwrap().clone();
    editor.pan_by(100., 50.);
    editor.zoom_about(0.5, point(0., 0.));
    paths.refresh(&editor, &shown);
    assert!(Rc::ptr_eq(paths.get("a").unwrap(), &a));
    editor.submit([Change::MoveNodes {
        moves: vec![("0".into(), (50, 20))],
    }]);
    paths.refresh(&editor, &shown);
    assert!(!Rc::ptr_eq(paths.get("a").unwrap(), &a));
    assert!(Rc::ptr_eq(paths.get("b").unwrap(), &b));
    let a = paths.get("a").unwrap().clone();
    paths.refresh(&editor, &Positions::from([("0".into(), (25., 10.))]));
    assert!(!Rc::ptr_eq(paths.get("a").unwrap(), &a));
    assert!(Rc::ptr_eq(paths.get("b").unwrap(), &b));
}

#[test]
fn routes_kinds_and_removed_edges_update_the_shared_paths() {
    let mut editor = editor();
    let mut paths = EdgePaths::default();
    let shown = Positions::new();
    paths.refresh(&editor, &shown);
    let a = paths.get("a").unwrap().clone();
    let mut edge = editor.canvas().edge("a").unwrap().clone();
    edge.points = vec![[100, 50], [150, 150], [300, 50]];
    editor.submit([Change::UpdateEdge { edge }]);
    paths.refresh(&editor, &shown);
    assert!(!Rc::ptr_eq(paths.get("a").unwrap(), &a));
    let a = paths.get("a").unwrap().clone();
    paths.refresh(&editor, &Positions::from([("0".into(), (20., 0.))]));
    assert!(!Rc::ptr_eq(paths.get("a").unwrap(), &a));
    let b = paths.get("b").unwrap().clone();
    editor.set_edge_kinds(EdgeKinds::new().with(crate::edge::CURVE, EdgeKind::new(path::line)));
    paths.refresh(&editor, &shown);
    assert!(!Rc::ptr_eq(paths.get("b").unwrap(), &b));
    editor.submit([Change::RemoveEdges {
        ids: vec!["a".into()],
    }]);
    paths.refresh(&editor, &shown);
    assert!(paths.get("a").is_none());
    assert!(paths.get("b").is_some());
}

#[test]
fn replacing_kinds_with_the_same_function_invalidates_paths() {
    let mut editor = editor();
    let mut paths = EdgePaths::default();
    let shown = Positions::new();
    paths.refresh(&editor, &shown);
    let before = paths.get("a").unwrap().clone();
    editor.set_edge_kinds(editor.edge_kinds().clone());
    paths.refresh(&editor, &shown);
    assert!(!Rc::ptr_eq(paths.get("a").unwrap(), &before));
    let before = paths.get("a").unwrap().clone();
    paths.refresh(&editor, &shown);
    assert!(Rc::ptr_eq(paths.get("a").unwrap(), &before));
}
