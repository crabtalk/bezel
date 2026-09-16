//! What a kind and a node let the reader do, honoured by every command.

use canvas::{
    Canvas, CanvasEditor, CanvasView, Handle, Item, Kinds,
    kind::{Capabilities, Capability, Kind},
    layout,
    layout::Arrow,
    model::Side,
};
use gpui::{
    IntoElement, Modifiers, MouseButton, TestAppContext, VisualTestContext, div, point, px, size,
};

/// `a` is an ordinary card; `locked` is one the document pins down; `wall` is
/// a kind there only to be read. The edge out of the wall will not be let go.
const DOC: &str = r#"{
  "nodes": [
    {"id":"a","type":"card","x":0,"y":0,"width":100,"height":40},
    {"id":"locked","type":"card","x":200,"y":0,"width":100,"height":40,"selectable":false,"draggable":false},
    {"id":"wall","type":"wall","x":0,"y":200,"width":100,"height":40}
  ],
  "edges": [{"id":"e","fromNode":"wall","toNode":"locked","deletable":false}]
}"#;

fn kinds() -> Kinds {
    let blank = || Kind::new(|_, _, _, _| div().into_any_element());
    Kinds::new()
        .with("card", blank())
        .with("wall", blank().can(Capabilities::READ_ONLY))
}

fn editor() -> CanvasEditor {
    CanvasEditor::new(Canvas::parse(DOC).unwrap(), layout::FREE).with_kinds(kinds())
}

#[test]
fn can_reads_the_kind_then_the_node() {
    let editor = editor();
    assert!(editor.can(&Item::Node("a".into()), Capability::Draggable));
    // The document refuses what the kind allows.
    assert!(!editor.can(&Item::Node("locked".into()), Capability::Draggable));
    // The kind refuses what the document says nothing about.
    assert!(!editor.can(&Item::Node("wall".into()), Capability::Resizable));
    assert!(editor.can(&Item::Node("wall".into()), Capability::Selectable));
    assert!(!editor.can(&Item::Edge("e".into()), Capability::Deletable));
    assert!(editor.can(&Item::Edge("e".into()), Capability::Selectable));
}

#[test]
fn what_cannot_be_selected_is_not() {
    let mut editor = editor();
    editor.select_all();
    assert_eq!(editor.selected_nodes(), ["a", "wall"]);
    editor.select(Some("locked".into()));
    assert_eq!(editor.selected(), None);
}

#[test]
fn what_cannot_be_dragged_does_not_nudge() {
    let mut editor = editor();
    editor.select_all();
    editor.nudge(Arrow::Right);
    let x = |id| editor.canvas().node(id).unwrap().x;
    assert_eq!((x("a"), x("wall"), x("locked")), (8, 0, 200));
}

#[test]
fn what_cannot_be_deleted_stays() {
    let mut editor = editor();
    editor.select_all();
    editor.remove_selected();
    let canvas = editor.canvas();
    assert!(canvas.node("a").is_none() && canvas.node("wall").is_some());

    editor.select_edge(Some("e".into()));
    editor.remove_selected();
    assert!(editor.canvas().edge("e").is_some());
}

#[test]
fn what_cannot_connect_draws_nothing() {
    let mut editor = editor();
    let edges = editor.canvas().edges.len();
    // Out of the wall, onto nothing, and onto a node.
    let right = Handle::connect(Side::Right);
    assert_eq!(editor.connect("wall", &right, (600, 600)), None);
    assert_eq!(editor.connect("a", &right, (30, 210)), None);
    assert_eq!(editor.canvas().edges.len(), edges);
}

#[gpui::test]
fn an_unresizable_node_has_no_corner_to_pull(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let doc = Canvas::parse(DOC).unwrap();
    let window =
        cx.add_window(move |_, cx| CanvasView::new(doc, layout::FREE, cx).with_kinds(kinds()));
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    let draw = |cx: &mut VisualTestContext| {
        for _ in 0..3 {
            cx.update(|window, cx| window.draw(cx).clear(cx));
        }
    };
    draw(&mut cx);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.update_editor(cx, |editor| editor.select(Some("wall".into())))
        })
    });
    draw(&mut cx);

    // Where the corner would be, had the wall one to pull.
    let (corner, zoom) = cx.update(|_, cx| {
        let view = view.read(cx);
        let (pan, zoom) = (view.editor().pan(), view.editor().zoom());
        let at = view.bounds().unwrap().origin
            + point(px(pan.x + 100.0 * zoom), px(pan.y + 240.0 * zoom));
        (at, zoom)
    });
    let to = corner + point(px(50.0 * zoom), px(20.0 * zoom));
    cx.simulate_mouse_down(corner, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());

    let wall = cx.update(|_, cx| view.read(cx).editor().canvas().node("wall").cloned());
    let wall = wall.expect("still there");
    assert_eq!((wall.width, wall.height), (100, 40), "the wall resized");
    assert_eq!((wall.x, wall.y), (0, 200), "the wall moved");
}
