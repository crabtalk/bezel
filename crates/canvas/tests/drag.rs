//! Dragging a node on a canvas with nothing around it.

use canvas::{Canvas, CanvasView, layout, mindmap};
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
    let window =
        cx.add_window(|_, cx| CanvasView::new(Canvas::parse(TREE).unwrap(), layout::MINDMAP, cx));
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

#[gpui::test]
fn selections_and_group_contents_reach_the_same_drag_frame(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let selection = r#"{"nodes":[{"id":"a","type":"link","x":100,"y":100,"width":200,"height":100},{"id":"b","type":"link","x":500,"y":300,"width":100,"height":100}]}"#;
    let group = r#"{"nodes":[{"id":"group","type":"group","x":100,"y":100,"width":300,"height":200},{"id":"child","type":"link","x":160,"y":140,"width":80,"height":40}]}"#;
    for (source, selected, from, probe, expected) in [
        (selection, vec!["a", "b"], (200., 150.), (629., 380.), "b"),
        (group, vec!["group"], (110., 110.), (269., 190.), "child"),
    ] {
        let window = cx.add_window(move |_, cx| {
            CanvasView::new(Canvas::parse(source).unwrap(), layout::FREE, cx)
        });
        let view = window.root(cx).unwrap();
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        visual.simulate_resize(size(px(800.), px(600.)));
        view.update(&mut visual, |view, cx| {
            view.update_editor(cx, |editor| {
                editor.set_selection(selected.into_iter().map(str::to_owned).collect());
            })
        });
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        let (start, end, probe) = visual.update(|_, cx| {
            let view = view.read(cx);
            let bounds = view.bounds().unwrap();
            let pan = view.editor().pan();
            let zoom = view.editor().zoom();
            let screen = |(x, y): (f32, f32)| {
                bounds.origin + point(px(pan.x + x * zoom), px(pan.y + y * zoom))
            };
            (
                screen(from),
                screen((from.0 + 30., from.1 + 30.)),
                screen(probe),
            )
        });
        visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        let item = visual.update(|_, cx| view.read(cx).item_at(probe));
        assert_eq!(item, Some(canvas::Item::Node(expected.into())));
        visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    }
}
