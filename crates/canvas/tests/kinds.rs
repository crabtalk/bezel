//! Kinds and the change filter, through the keys a reader presses.

use canvas::{
    Canvas, CanvasView, Change, Kinds,
    kind::{Field, Kind},
    mindmap,
    model::Node,
};
use gpui::{Entity, Focusable, IntoElement, TestAppContext, VisualTestContext, div, px, size};

const DOC: &str = r#"{
  "nodes": [
    {"id":"root","type":"card","x":0,"y":0,"width":80,"height":30,"label":"root"},
    {"id":"leaf","type":"card","x":0,"y":0,"width":80,"height":30,"label":"leaf"}
  ],
  "edges": [{"id":"e","fromNode":"root","toNode":"leaf"}]
}"#;

fn card() -> Kind {
    Kind::new(|_, _, _, _| div().into_any_element())
        .edit(Field::new(
            |node| node.label.clone().unwrap_or_default(),
            |node, label| node.label = Some(label),
        ))
        .child(|_| Node {
            kind: "card".into(),
            width: 80,
            height: 30,
            ..Node::default()
        })
}

fn open(
    view: impl FnOnce(CanvasView) -> CanvasView + 'static,
    cx: &mut TestAppContext,
) -> (Entity<CanvasView>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
        canvas::set_kinds(cx, Kinds::new().with("card", card()));
    });
    let window = cx.add_window(|_, cx| view(CanvasView::new(Canvas::parse(DOC).unwrap(), cx)));
    let root = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    // A test window has no frame clock: draw until layout has settled.
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    (root, cx)
}

fn select(view: &Entity<CanvasView>, id: &str, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.update_editor(cx, |editor| editor.select(Some(id.into())))
        });
        let handle = view.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
}

#[gpui::test]
fn tab_makes_the_kinds_child(cx: &mut TestAppContext) {
    let (view, mut cx) = open(|view| view, cx);
    select(&view, "leaf", &mut cx);
    cx.simulate_keystrokes("tab");
    let canvas = cx.update(|_, cx| view.read(cx).editor().canvas().clone());
    let made = canvas.nodes.last().unwrap();
    assert_eq!((made.kind.as_str(), made.width), ("card", 80));
    assert_eq!(mindmap::parent(&canvas, &made.id), Some("leaf"));
}

#[gpui::test]
fn a_filter_refuses_and_rewrites(cx: &mut TestAppContext) {
    let (view, mut cx) = open(
        |view| {
            view.with_changes(|canvas, change| match change {
                // Roots stay.
                Change::RemoveNodes { ids }
                    if ids.iter().any(|id| mindmap::parent(canvas, id).is_none()) =>
                {
                    None
                }
                // Every new card is born wide.
                Change::AddNode { mut node, index } => {
                    node.width = 300;
                    Some(Change::AddNode { node, index })
                }
                other => Some(other),
            })
        },
        cx,
    );
    select(&view, "root", &mut cx);
    cx.simulate_keystrokes("backspace");
    assert!(cx.update(|_, cx| view.read(cx).editor().canvas().node("root").is_some()));

    cx.simulate_keystrokes("tab");
    let width = cx.update(|_, cx| view.read(cx).editor().canvas().nodes.last().unwrap().width);
    assert_eq!(width, 300);

    select(&view, "leaf", &mut cx);
    cx.simulate_keystrokes("backspace");
    assert!(cx.update(|_, cx| view.read(cx).editor().canvas().node("leaf").is_none()));
}
