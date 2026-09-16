//! Dragging a node on a canvas with nothing around it.

use canvas::{Canvas, CanvasView, mindmap};
use gpui::{
    Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px, size,
};

const TREE: &str = r#"{
  "nodes": [
    {"id":"root","type":"text","x":0,"y":0,"width":200,"height":40,"text":"root"},
    {"id":"child","type":"text","x":0,"y":0,"width":200,"height":40,"text":"child"}
  ],
  "edges": [{"id":"e","fromNode":"root","toNode":"child"}]
}"#;

fn middle(view: &CanvasView, id: &str) -> Point<Pixels> {
    let (bounds, pan, zoom) = (
        view.bounds().expect("painted"),
        view.editor().pan(),
        view.editor().zoom(),
    );
    let node = view.editor().canvas().node(id).expect("in the tree");
    bounds.origin
        + point(
            px(pan.x + (node.x + node.width / 2) as f32 * zoom),
            px(pan.y + (node.y + node.height / 2) as f32 * zoom),
        )
}

#[gpui::test]
fn a_child_drags_and_stays(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let window = cx.add_window(|_, cx| CanvasView::new(Canvas::parse(TREE).unwrap(), cx));
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    cx.run_until_parked();
    // A test window has no frame clock: draw until the resize, the centring and
    // the measured heights have all painted.
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    let (at, origin) = cx.update(|_, cx| {
        let view = view.read(cx);
        let child = view.editor().canvas().node("child").unwrap();
        (middle(view, "child"), (child.x, child.y))
    });
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    let selected = cx.update(|_, cx| view.read(cx).editor().selected().map(str::to_owned));
    assert_eq!(selected.as_deref(), Some("child"), "the press missed");
    let to = at + point(px(30.0), px(50.0));
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    let child = cx
        .update(|_, cx| view.read(cx).editor().canvas().node("child").cloned())
        .unwrap();
    assert_eq!((child.x, child.y), (origin.0 + 30, origin.1 + 50));
    assert!(mindmap::is_pinned(&child));
}
