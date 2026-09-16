//! The editor without a window: history, the selection, the filter, the part
//! in view, layouts and kinds, driven by its commands.

use canvas::{
    Canvas, CanvasEditor, Change, Item, Kinds,
    kind::Kind,
    layout::{self, Arrow},
    mindmap,
    model::Node,
};
use gpui::{IntoElement, div, size};

/// Three apart.
const SCATTER: &str = r#"{
  "nodes": [
    {"id":"a","type":"text","x":0,"y":0,"width":200,"height":40,"text":"a"},
    {"id":"b","type":"text","x":300,"y":0,"width":200,"height":40,"text":"b"},
    {"id":"c","type":"text","x":0,"y":300,"width":200,"height":40,"text":"c"}
  ],
  "edges": [{"id":"e","fromNode":"a","toNode":"b"}]
}"#;

/// Three far apart.
const WIDE: &str = r#"{
  "nodes": [
    {"id":"a","type":"link","x":0,"y":0,"width":200,"height":40,"url":"a"},
    {"id":"b","type":"link","x":3000,"y":0,"width":200,"height":40,"url":"b"},
    {"id":"c","type":"link","x":1500,"y":1200,"width":200,"height":40,"url":"c"}
  ]
}"#;

/// root → child, the child dragged off and pinned there.
const PINNED: &str = r#"{
  "nodes": [
    {"id":"root","type":"text","x":0,"y":0,"width":200,"height":40,"text":"root"},
    {"id":"child","type":"text","x":900,"y":700,"width":200,"height":40,"text":"child","pinned":true}
  ],
  "edges": [{"id":"e","fromNode":"root","toNode":"child"}]
}"#;

fn free(json: &str) -> CanvasEditor {
    CanvasEditor::new(Canvas::parse(json).unwrap()).with_layout(layout::FREE)
}

#[test]
fn nudges_undo_and_redo() {
    let mut editor = free(SCATTER);
    editor.select(Some("a".into()));
    editor.nudge(Arrow::Right);
    editor.nudge(Arrow::Right);
    let x = |editor: &CanvasEditor| editor.canvas().node("a").unwrap().x;
    assert_eq!(x(&editor), 16);
    assert!(editor.undo());
    assert_eq!(x(&editor), 8);
    assert!(editor.undo() && !editor.undo());
    assert_eq!(x(&editor), 0);
    assert!(editor.redo());
    assert_eq!(x(&editor), 8);
}

#[test]
fn a_free_canvas_removes_only_the_node() {
    let mut editor = free(PINNED);
    editor.select(Some("root".into()));
    editor.remove_selected();
    let canvas = editor.canvas();
    assert!(canvas.node("root").is_none() && canvas.node("child").is_some());
    assert!(canvas.edges.is_empty());
    assert!(editor.undo());
    assert_eq!(editor.canvas(), &Canvas::parse(PINNED).unwrap());
}

#[test]
fn a_filter_refuses_the_whole_batch() {
    let mut editor = free(SCATTER).with_changes(|_, change| match change {
        Change::RemoveNodes { .. } => None,
        other => Some(other),
    });
    editor.set_selection(vec!["a".into(), "b".into()]);
    editor.remove_selected();
    assert_eq!(editor.canvas().nodes.len(), 3);
    assert!(!editor.can_undo());
}

#[test]
fn the_selection_is_nodes_or_one_edge() {
    let mut editor = free(SCATTER);
    editor.set_selection(vec!["a".into(), "b".into()]);
    assert_eq!(editor.selected(), Some("b"));
    editor.select_edge(Some("e".into()));
    assert_eq!(editor.selection(), [Item::Edge("e".into())]);
    assert_eq!(editor.selected(), None);
    editor.select(Some("c".into()));
    assert_eq!(editor.selected_edge(), None);
    assert_eq!(editor.selection(), [Item::Node("c".into())]);
}

/// Whether node `id` lies wholly in view.
fn in_view(editor: &CanvasEditor, id: &str) -> bool {
    let (x, y, w, h) = editor.visible().unwrap();
    let node = editor.canvas().node(id).unwrap();
    let (left, top) = (node.x as f32, node.y as f32);
    left >= x && top >= y && left + node.width as f32 <= x + w && top + node.height as f32 <= y + h
}

#[test]
fn fit_shows_everything() {
    let mut editor = free(WIDE);
    editor.set_viewport(size(800.0, 600.0));
    editor.frame();
    assert!(!in_view(&editor, "b"));
    editor.fit();
    assert!(editor.zoom() < 1.0);
    for id in ["a", "b", "c"] {
        assert!(in_view(&editor, id), "{id} is out of view");
    }
}

#[test]
fn zoom_to_selection_centres_it() {
    let mut editor = free(WIDE);
    editor.set_viewport(size(800.0, 600.0));
    editor.select(Some("c".into()));
    editor.zoom_to_selection();
    assert!(in_view(&editor, "c"));
    let (x, y, w, h) = editor.visible().unwrap();
    assert!((x + w / 2.0 - 1600.0).abs() < 1.0 && (y + h / 2.0 - 1220.0).abs() < 1.0);
}

#[test]
fn center_on_puts_a_point_in_the_middle() {
    let mut editor = free(WIDE);
    editor.set_viewport(size(800.0, 600.0));
    editor.center_on((1500.0, 600.0));
    let (x, y, w, h) = editor.visible().unwrap();
    assert_eq!((x + w / 2.0, y + h / 2.0), (1500.0, 600.0));
}

#[test]
fn a_tree_layout_drops_the_pins_and_lays_out() {
    let mut editor = free(PINNED);
    editor.set_layout(layout::MINDMAP);
    editor.reflow([], None);
    let child = editor.canvas().node("child").unwrap();
    assert!(!mindmap::is_pinned(child));
    let mut laid = editor.canvas().clone();
    mindmap::layout(&mut laid);
    let expected = laid.node("child").unwrap();
    assert_eq!((child.x, child.y), (expected.x, expected.y));
}

fn card(width: i64) -> Kind {
    Kind::new(|_, _, _, _| div().into_any_element()).child(move |_| Node {
        kind: "card".into(),
        width,
        height: 30,
        ..Node::default()
    })
}

#[test]
fn each_editor_makes_with_its_own_kinds() {
    let doc = Canvas::parse(
        r#"{"nodes":[{"id":"root","type":"card","x":0,"y":0,"width":80,"height":30}]}"#,
    )
    .unwrap();
    for width in [80, 300] {
        let mut editor =
            CanvasEditor::new(doc.clone()).with_kinds(Kinds::new().with("card", card(width)));
        editor.select(Some("root".into()));
        let made = editor.add_child().expect("tab adds");
        assert_eq!(editor.canvas().node(&made).unwrap().width, width);
        assert_eq!(editor.selected(), Some(made.as_str()));
    }
}
