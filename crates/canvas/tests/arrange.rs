//! Turning auto layout back on tidies what a free canvas left.

use canvas::{Arrange, Canvas, CanvasView, mindmap};
use gpui::{TestAppContext, VisualTestContext, px, size};

/// root → child, the child dragged off and pinned there.
const DOC: &str = r#"{
  "nodes": [
    {"id":"root","type":"text","x":0,"y":0,"width":200,"height":40,"text":"root"},
    {"id":"child","type":"text","x":900,"y":700,"width":200,"height":40,"text":"child","pinned":true}
  ],
  "edges": [{"id":"e","fromNode":"root","toNode":"child"}]
}"#;

#[gpui::test]
fn auto_layout_drops_the_pins_and_lays_out(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let window = cx.add_window(|_, cx| {
        CanvasView::new(Canvas::parse(DOC).unwrap(), cx).with_arrange(Arrange::Free)
    });
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    cx.update(|window, cx| window.draw(cx).clear(cx));

    cx.update(|_, cx| view.update(cx, |view, cx| view.set_arrange(Arrange::Mindmap, cx)));
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    let canvas = cx.update(|_, cx| view.read(cx).canvas().clone());
    let child = canvas.node("child").unwrap();
    assert!(!mindmap::is_pinned(child));
    let mut laid = canvas.clone();
    mindmap::layout(&mut laid);
    let expected = laid.node("child").unwrap();
    assert_eq!((child.x, child.y), (expected.x, expected.y));
}

#[gpui::test]
fn a_free_canvas_removes_only_the_node(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let window = cx.add_window(|_, cx| {
        CanvasView::new(Canvas::parse(DOC).unwrap(), cx).with_arrange(Arrange::Free)
    });
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.select(Some("root".into()), cx);
            view.remove_selected(cx);
        })
    });
    let canvas = cx.update(|_, cx| view.read(cx).canvas().clone());
    assert!(canvas.node("root").is_none() && canvas.node("child").is_some());
    assert!(canvas.edges.is_empty());
}
