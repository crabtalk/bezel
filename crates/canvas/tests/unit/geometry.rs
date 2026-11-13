use crate::{
    change::Change,
    document::Document,
    edit::CanvasEditor,
    kind, layout,
    model::{Canvas, GROUP, LINK, Node},
    view::geometry::Geometry,
};
use gpui::{point, size};
use std::{hint, time::Instant};

fn editor() -> CanvasEditor {
    CanvasEditor::new(
        Canvas {
            nodes: vec![
                Node {
                    id: "child".into(),
                    kind: LINK.into(),
                    x: 20,
                    y: 20,
                    width: 100,
                    height: 100,
                    ..Node::default()
                },
                Node {
                    id: "group".into(),
                    kind: GROUP.into(),
                    width: 400,
                    height: 400,
                    ..Node::default()
                },
                Node {
                    id: "other".into(),
                    kind: LINK.into(),
                    x: 500,
                    width: 100,
                    height: 100,
                    ..Node::default()
                },
            ],
            ..Canvas::default()
        },
        layout::FREE,
    )
}

#[test]
fn camera_changes_retain_geometry_and_carried_nodes_stay_on_top() {
    let mut editor = editor();
    let mut geometry = Geometry::default();
    assert!(geometry.refresh(&editor, None));
    assert_eq!(geometry.order, [1, 2, 0]);
    editor.set_viewport(size(800., 600.));
    editor.pan_by(100., 50.);
    editor.zoom_about(0.5, point(200., 100.));
    assert!(!geometry.refresh(&editor, None));
    assert!(geometry.refresh(&editor, Some("group")));
    assert_eq!(geometry.order, [2, 1, 0]);
    assert!(!geometry.refresh(&editor, Some("group")));
    assert!(geometry.refresh(&editor, None));
    assert_eq!(geometry.order, [1, 2, 0]);
}

#[test]
fn edits_previews_history_and_kinds_invalidate_geometry() {
    let mut editor = editor();
    let mut geometry = Geometry::default();
    geometry.refresh(&editor, None);
    editor.submit([Change::MoveNodes {
        moves: vec![("child".into(), (600, 50))],
    }]);
    assert!(geometry.refresh(&editor, None));
    assert_eq!(geometry.positions["child"], (600., 50.));
    editor.undo();
    assert!(geometry.refresh(&editor, None));
    assert_eq!(geometry.positions["child"], (20., 20.));
    editor.set_preview(
        vec![Change::MoveNodes {
            moves: vec![("child".into(), (700, 80))],
        }],
        vec![],
        vec![],
    );
    assert!(geometry.refresh(&editor, None));
    assert_eq!(geometry.positions["child"], (700., 80.));
    editor.clear_preview();
    assert!(geometry.refresh(&editor, None));
    assert_eq!(geometry.positions["child"], (20., 20.));
    editor.set_kinds(editor.kinds().clone().with(GROUP, kind::unknown()));
    assert!(geometry.refresh(&editor, None));
    assert_eq!(geometry.order, [0, 1, 2]);
    editor.submit([Change::RemoveNodes {
        ids: vec!["child".into()],
    }]);
    assert!(geometry.refresh(&editor, None));
    assert!(!geometry.positions.contains_key("child"));
    assert_eq!(geometry.order, [0, 1]);
}

#[test]
#[ignore]
fn camera_geometry_workload() {
    let mut editor = CanvasEditor::new(
        Canvas {
            nodes: (0..5000)
                .map(|index| Node {
                    id: index.to_string(),
                    kind: LINK.into(),
                    width: 100,
                    height: 100,
                    ..Node::default()
                })
                .collect(),
            ..Canvas::default()
        },
        layout::FREE,
    );
    let mut geometry = Geometry::default();
    geometry.refresh(&editor, None);
    let start = Instant::now();
    for _ in 0..1000 {
        editor.pan_by(1., 0.);
        hint::black_box(geometry.refresh(&editor, None));
    }
    eprintln!(
        "5,000 nodes, 1,000 camera geometry checks: {:?}",
        start.elapsed()
    );
}

#[test]
fn swapping_documents_with_equal_revisions_invalidates_geometry() {
    let mut editor = editor();
    let mut geometry = Geometry::default();
    geometry.refresh(&editor, None);
    let mut replacement = editor.canvas().clone();
    replacement.node_mut("child").unwrap().x = 900;
    editor.take_document(Document::new(replacement.clone()));
    assert!(geometry.refresh(&editor, None));
    assert_eq!(geometry.positions["child"].0, 900.);
    replacement.node_mut("child").unwrap().x = 1200;
    editor.set_canvas(replacement);
    assert!(geometry.refresh(&editor, None));
    assert_eq!(geometry.positions["child"].0, 1200.);
}
