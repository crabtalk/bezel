//! Layouts through the view: switching, and what a free canvas does with keys
//! and clicks.

use canvas::{Canvas, CanvasView, layout, mindmap};
use gpui::{
    Entity, Focusable, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext,
    VisualTestContext, point, px, size,
};

/// root → child, the child dragged off and pinned there.
const DOC: &str = r#"{
  "nodes": [
    {"id":"root","type":"text","x":0,"y":0,"width":200,"height":40,"text":"root"},
    {"id":"child","type":"text","x":900,"y":700,"width":200,"height":40,"text":"child","pinned":true}
  ],
  "edges": [{"id":"e","fromNode":"root","toNode":"child"}]
}"#;

fn open(cx: &mut TestAppContext) -> (Entity<CanvasView>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let window = cx.add_window(|_, cx| {
        CanvasView::new(Canvas::parse(DOC).unwrap(), cx).with_layout(layout::FREE)
    });
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    (view, cx)
}

#[gpui::test]
fn a_tree_layout_drops_the_pins_and_lays_out(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    cx.update(|_, cx| view.update(cx, |view, cx| view.set_layout(layout::MINDMAP, cx)));
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
    let (view, mut cx) = open(cx);
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

#[gpui::test]
fn shift_arrows_nudge_and_arrows_find_the_nearest(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| view.select(Some("root".into()), cx));
        let handle = view.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    cx.simulate_keystrokes("shift-right shift-down");
    let root = cx.update(|_, cx| view.read(cx).canvas().node("root").cloned().unwrap());
    assert_eq!((root.x, root.y), (8, 8));
    assert!(!mindmap::is_pinned(&root));

    // The child is further across than down, so it is to the right.
    cx.simulate_keystrokes("down");
    let selected = cx.update(|_, cx| view.read(cx).selected().map(str::to_owned));
    assert_eq!(selected.as_deref(), Some("root"));
    cx.simulate_keystrokes("right");
    let selected = cx.update(|_, cx| view.read(cx).selected().map(str::to_owned));
    assert_eq!(selected.as_deref(), Some("child"));
}

#[gpui::test]
fn a_double_click_on_nothing_adds_a_node_there(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    let (at, want) = cx.update(|_, cx| {
        let view = view.read(cx);
        let (bounds, pan, zoom) = (view.bounds().unwrap(), view.pan(), view.zoom());
        let local = point(400.0, 300.0);
        let want = (
            ((local.x - pan.x) / zoom).round() as i64,
            ((local.y - pan.y) / zoom).round() as i64,
        );
        (bounds.origin + point(px(local.x), px(local.y)), want)
    });
    for click_count in 1..=2 {
        cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count,
        });
    }
    let canvas = cx.update(|_, cx| view.read(cx).canvas().clone());
    assert_eq!(canvas.nodes.len(), 3);
    let made = canvas.nodes.last().unwrap();
    // Centred on the click as made; measuring its text grows it down.
    assert_eq!(made.x + made.width / 2, want.0);
    assert!((made.y..made.y + made.height).contains(&want.1), "{made:?}");
    let selected = cx.update(|_, cx| view.read(cx).selected().map(str::to_owned));
    assert_eq!(selected.as_deref(), Some(made.id.as_str()));
}
