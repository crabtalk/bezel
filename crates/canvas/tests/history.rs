//! Undo, the selection and the clipboard: pure, then through the keys.

use std::collections::HashSet;

use canvas::{
    Canvas, CanvasView, Change, Layout, change, clip, layout, mindmap,
    model::{Edge, Node, TEXT},
};
use gpui::{
    Entity, Focusable, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext,
    point, px, size,
};

fn node(id: &str) -> Node {
    Node {
        id: id.into(),
        kind: TEXT.into(),
        width: 100,
        height: 40,
        text: Some(id.into()),
        ..Node::default()
    }
}

/// root → a, b; a → a1; and a cross link from b to a1.
fn tree() -> Canvas {
    let mut link = Edge::new("x", "b", "a1");
    link.extra.insert(mindmap::TREE.into(), false.into());
    Canvas {
        nodes: ["root", "a", "b", "a1"].map(node).into(),
        edges: vec![
            Edge::new("e1", "root", "a"),
            Edge::new("e2", "root", "b"),
            Edge::new("e3", "a", "a1"),
            link,
        ],
        ..Canvas::default()
    }
}

#[test]
fn a_batch_undoes_to_where_it_began() {
    let before = tree();
    let mut canvas = before.clone();
    let batch = [
        mindmap::remove(&canvas, "a"),
        Change::MoveNodes {
            moves: vec![("b".into(), (5, 6))],
        },
        Change::AddNode {
            node: node("c"),
            index: None,
        },
    ];
    let undo = change::apply_all(&mut canvas, &batch);
    assert!(canvas.node("a1").is_none());
    let redo = change::apply_all(&mut canvas, &undo);
    assert_eq!(canvas, before);
    change::apply_all(&mut canvas, &redo);
    let mut after = before.clone();
    change::apply_all(&mut after, &batch);
    assert_eq!(canvas, after);
}

#[test]
fn a_paste_mints_ids_and_hangs_its_roots() {
    let canvas = tree();
    let fragment = clip::fragment(&canvas, &["a".into(), "a1".into(), "b".into()]);
    assert_eq!(fragment.edges.len(), 2);
    let changes = clip::paste(&canvas, &fragment, (500, 500), Some("root"));
    let mut pasted = canvas.clone();
    change::apply_all(&mut pasted, &changes);
    assert_eq!(pasted.nodes.len(), 7);
    let ids: HashSet<&str> = pasted
        .nodes
        .iter()
        .map(|n| n.id.as_str())
        .chain(pasted.edges.iter().map(|e| e.id.as_str()))
        .collect();
    assert_eq!(ids.len(), pasted.nodes.len() + pasted.edges.len());
    // a and b's copies hang under root; a1's copy stays under a's.
    assert_eq!(mindmap::children(&pasted, "root").count(), 4);
    assert!(pasted.nodes[4..].iter().all(|n| (n.x, n.y) == (500, 500)));
}

/// root → a → a1, root → b
const TREE: &str = r#"{
  "nodes": [
    {"id":"root","type":"text","x":0,"y":0,"width":200,"height":40,"text":"root"},
    {"id":"a","type":"text","x":0,"y":0,"width":200,"height":40,"text":"a"},
    {"id":"a1","type":"text","x":0,"y":0,"width":200,"height":40,"text":"a1"},
    {"id":"b","type":"text","x":0,"y":0,"width":200,"height":40,"text":"b"}
  ],
  "edges": [
    {"id":"e1","fromNode":"root","toNode":"a"},
    {"id":"e2","fromNode":"a","toNode":"a1"},
    {"id":"e3","fromNode":"root","toNode":"b"}
  ]
}"#;

/// Three apart, on a free canvas.
const SCATTER: &str = r#"{
  "nodes": [
    {"id":"a","type":"text","x":0,"y":0,"width":200,"height":40,"text":"a"},
    {"id":"b","type":"text","x":300,"y":0,"width":200,"height":40,"text":"b"},
    {"id":"c","type":"text","x":0,"y":300,"width":200,"height":40,"text":"c"}
  ]
}"#;

fn open(
    json: &str,
    layout: Layout,
    cx: &mut TestAppContext,
) -> (Entity<CanvasView>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let doc = Canvas::parse(json).unwrap();
    let window = cx.add_window(move |_, cx| CanvasView::new(doc, cx).with_layout(layout));
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    focus(&view, &mut cx);
    (view, cx)
}

fn focus(view: &Entity<CanvasView>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        let handle = view.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
}

/// `keys` with `cmd` as this platform's.
fn press(keys: &str, cx: &mut VisualTestContext) {
    let platform = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.simulate_keystrokes(&keys.replace("cmd", platform));
    cx.run_until_parked();
}

fn doc(view: &Entity<CanvasView>, cx: &mut VisualTestContext) -> Canvas {
    cx.update(|_, cx| view.read(cx).canvas().clone())
}

fn selection(view: &Entity<CanvasView>, cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| view.read(cx).selection().to_vec())
}

/// A canvas point, in window coordinates.
fn screen(view: &Entity<CanvasView>, at: (i64, i64), cx: &mut VisualTestContext) -> Point<Pixels> {
    cx.update(|_, cx| {
        let view = view.read(cx);
        let (bounds, pan, zoom) = (view.bounds().unwrap(), view.pan(), view.zoom());
        bounds.origin
            + point(
                px(pan.x + at.0 as f32 * zoom),
                px(pan.y + at.1 as f32 * zoom),
            )
    })
}

fn click(at: Point<Pixels>, modifiers: Modifiers, cx: &mut VisualTestContext) {
    cx.simulate_mouse_down(at, MouseButton::Left, modifiers);
    cx.simulate_mouse_up(at, MouseButton::Left, modifiers);
}

fn shift() -> Modifiers {
    Modifiers {
        shift: true,
        ..Modifiers::default()
    }
}

#[gpui::test]
fn undo_takes_back_an_edit_and_redo_puts_it_back(cx: &mut TestAppContext) {
    let (view, mut cx) = open(SCATTER, layout::FREE, cx);
    cx.update(|_, cx| view.update(cx, |view, cx| view.select(Some("a".into()), cx)));
    press("shift-right shift-right", &mut cx);
    assert_eq!(doc(&view, &mut cx).node("a").unwrap().x, 16);
    press("cmd-z", &mut cx);
    assert_eq!(doc(&view, &mut cx).node("a").unwrap().x, 8);
    press("cmd-z cmd-z", &mut cx);
    assert_eq!(doc(&view, &mut cx).node("a").unwrap().x, 0);
    press("cmd-shift-z", &mut cx);
    assert_eq!(doc(&view, &mut cx).node("a").unwrap().x, 8);
}

#[gpui::test]
fn shift_clicks_gather_a_selection_that_goes_together(cx: &mut TestAppContext) {
    let (view, mut cx) = open(SCATTER, layout::FREE, cx);
    let a = screen(&view, (100, 20), &mut cx);
    let b = screen(&view, (400, 20), &mut cx);
    click(a, Modifiers::none(), &mut cx);
    click(b, shift(), &mut cx);
    assert_eq!(selection(&view, &mut cx), ["a", "b"]);

    press("backspace", &mut cx);
    let ids: Vec<String> = doc(&view, &mut cx)
        .nodes
        .into_iter()
        .map(|n| n.id)
        .collect();
    assert_eq!(ids, ["c"]);
    press("cmd-z", &mut cx);
    // Back where they were; their heights are measured, not undone.
    let places = |canvas: Canvas| -> Vec<(String, i64, i64)> {
        canvas.nodes.into_iter().map(|n| (n.id, n.x, n.y)).collect()
    };
    assert_eq!(
        places(doc(&view, &mut cx)),
        places(Canvas::parse(SCATTER).unwrap())
    );
    press("cmd-a", &mut cx);
    assert_eq!(selection(&view, &mut cx).len(), 3);
    press("escape", &mut cx);
    assert!(selection(&view, &mut cx).is_empty());
}

#[gpui::test]
fn a_shift_drag_on_nothing_selects_what_it_touches(cx: &mut TestAppContext) {
    let (view, mut cx) = open(SCATTER, layout::FREE, cx);
    let (from, to) = (
        screen(&view, (-20, -20), &mut cx),
        screen(&view, (320, 60), &mut cx),
    );
    cx.simulate_mouse_down(from, MouseButton::Left, shift());
    cx.simulate_mouse_move(to, MouseButton::Left, shift());
    cx.simulate_mouse_up(to, MouseButton::Left, shift());
    assert_eq!(selection(&view, &mut cx), ["a", "b"]);
}

#[gpui::test]
fn a_copied_branch_pastes_under_the_selection(cx: &mut TestAppContext) {
    let (view, mut cx) = open(TREE, layout::MINDMAP, cx);
    cx.update(|_, cx| view.update(cx, |view, cx| view.select(Some("a".into()), cx)));
    press("cmd-c", &mut cx);
    cx.update(|_, cx| view.update(cx, |view, cx| view.select(Some("b".into()), cx)));
    press("cmd-v", &mut cx);

    let canvas = doc(&view, &mut cx);
    assert_eq!(canvas.nodes.len(), 6);
    let copy = mindmap::children(&canvas, "b")
        .next()
        .expect("pasted under b");
    let text = |id: &str| canvas.node(id).unwrap().text.clone().unwrap();
    assert_eq!(text(copy), "a");
    let below = mindmap::children(&canvas, copy)
        .next()
        .expect("its branch came too");
    assert_eq!(text(below), "a1");
    assert_eq!(selection(&view, &mut cx), [copy, below]);

    press("cmd-z", &mut cx);
    assert_eq!(doc(&view, &mut cx).nodes.len(), 4);
}
