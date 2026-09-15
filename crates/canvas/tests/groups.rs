//! Groups frame what sits inside them: members go where the group goes, and a
//! group is never a drop target.

use canvas::{
    Canvas, CanvasView, change, group, kind, layout, mindmap,
    model::{GROUP, Node, TEXT},
};
use gpui::{
    Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px,
    size,
};

fn node(id: &str, kind: &str, (x, y, width, height): (i64, i64, i64, i64)) -> Node {
    Node {
        id: id.into(),
        kind: kind.into(),
        x,
        y,
        width,
        height,
        ..Node::default()
    }
}

/// g frames a; b sits outside.
fn framed() -> Canvas {
    Canvas {
        nodes: vec![
            node("g", GROUP, (0, 0, 400, 300)),
            node("a", TEXT, (20, 40, 100, 40)),
            node("b", TEXT, (500, 0, 100, 40)),
        ],
        ..Canvas::default()
    }
}

fn at(canvas: &Canvas, id: &str) -> (i64, i64) {
    let node = canvas.node(id).unwrap();
    (node.x, node.y)
}

#[test]
fn a_group_carries_what_it_frames() {
    let mut canvas = framed();
    assert_eq!(group::members(&canvas, "g"), ["a"]);
    let changes = mindmap::carry(&canvas, &["g".into()], (10, 20), false);
    change::apply_all(&mut canvas, &changes);
    assert_eq!(
        [at(&canvas, "g"), at(&canvas, "a"), at(&canvas, "b")],
        [(10, 20), (30, 60), (500, 0)]
    );
}

#[test]
fn a_group_is_no_drop_target() {
    let canvas = framed();
    assert_eq!(mindmap::node_at(&canvas, (300, 200), &[]), None);
    assert_eq!(mindmap::node_at(&canvas, (50, 50), &[]), Some("a"));
}

#[test]
fn images_are_known_by_their_extension() {
    assert!(kind::is_image("art/cover.PNG"));
    assert!(!kind::is_image("notes/a.md"));
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

#[gpui::test]
fn dragging_a_group_moves_its_members(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let window = cx.add_window(|_, cx| CanvasView::new(framed(), cx).with_layout(layout::FREE));
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    let zoom = cx.update(|_, cx| view.read(cx).zoom());
    let from = screen(&view, (300, 250), &mut cx);
    let to = from + point(px(60.0 * zoom), px(40.0 * zoom));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());

    let canvas = cx.update(|_, cx| view.read(cx).canvas().clone());
    assert_eq!(
        [at(&canvas, "g"), at(&canvas, "a"), at(&canvas, "b")],
        [(60, 40), (80, 80), (500, 0)]
    );
}
