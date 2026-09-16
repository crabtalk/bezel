//! A gesture in hand paints over the document and leaves it alone until it
//! lands.

use canvas::{Canvas, CanvasView, layout};
use gpui::{
    Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px,
    size,
};

/// Two links, which keep their size.
const PAIR: &str = r#"{
  "nodes": [
    {"id":"a","type":"link","x":0,"y":0,"width":200,"height":40,"url":"a"},
    {"id":"b","type":"link","x":400,"y":300,"width":200,"height":40,"url":"b"}
  ]
}"#;

fn open(refuse: bool, cx: &mut TestAppContext) -> (Entity<CanvasView>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let doc = Canvas::parse(PAIR).unwrap();
    let window = cx.add_window(move |_, cx| {
        CanvasView::new(doc, cx)
            .with_layout(layout::FREE)
            .with_changes(move |_, change| (!refuse).then_some(change))
    });
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    draw(&mut cx);
    (view, cx)
}

fn draw(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
}

/// A canvas point, in window coordinates, and the zoom.
fn screen(
    view: &Entity<CanvasView>,
    at: (i64, i64),
    cx: &mut VisualTestContext,
) -> (Point<Pixels>, f32) {
    cx.update(|_, cx| {
        let view = view.read(cx);
        let (pan, zoom) = (view.editor().pan(), view.editor().zoom());
        let at = view.bounds().unwrap().origin
            + point(
                px(pan.x + at.0 as f32 * zoom),
                px(pan.y + at.1 as f32 * zoom),
            );
        (at, zoom)
    })
}

/// A node's box in the document, and where it paints.
type Boxes = ((i64, i64, i64, i64), (i64, i64, i64, i64));

fn boxes(view: &Entity<CanvasView>, id: &str, cx: &mut VisualTestContext) -> Boxes {
    cx.update(|_, cx| {
        let editor = view.read(cx).editor();
        let of = |canvas: &Canvas| {
            let node = canvas.node(id).unwrap();
            (node.x, node.y, node.width, node.height)
        };
        (of(editor.canvas()), of(editor.painted()))
    })
}

#[gpui::test]
fn a_drag_paints_over_the_document_until_it_drops(cx: &mut TestAppContext) {
    let (view, mut cx) = open(false, cx);
    let (from, zoom) = screen(&view, (100, 20), &mut cx);
    let to = from + point(px(60.0 * zoom), px(40.0 * zoom));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    assert_eq!(
        boxes(&view, "a", &mut cx),
        ((0, 0, 200, 40), (60, 40, 200, 40))
    );
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    assert_eq!(
        boxes(&view, "a", &mut cx),
        ((60, 40, 200, 40), (60, 40, 200, 40))
    );
}

#[gpui::test]
fn a_refused_drop_changes_nothing(cx: &mut TestAppContext) {
    let (view, mut cx) = open(true, cx);
    let (from, zoom) = screen(&view, (100, 20), &mut cx);
    let to = from + point(px(60.0 * zoom), px(40.0 * zoom));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    assert_eq!(
        boxes(&view, "a", &mut cx),
        ((0, 0, 200, 40), (0, 0, 200, 40))
    );
    assert!(!cx.update(|_, cx| view.read(cx).editor().can_undo()));
}

#[gpui::test]
fn a_resize_paints_over_the_document_until_it_lands(cx: &mut TestAppContext) {
    let (view, mut cx) = open(false, cx);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.update_editor(cx, |editor| editor.select(Some("b".into())))
        })
    });
    draw(&mut cx);
    let (corner, zoom) = screen(&view, (600, 340), &mut cx);
    let to = corner + point(px(50.0 * zoom), px(20.0 * zoom));
    cx.simulate_mouse_down(corner, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    assert_eq!(
        boxes(&view, "b", &mut cx),
        ((400, 300, 200, 40), (400, 300, 250, 60))
    );
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    assert_eq!(
        boxes(&view, "b", &mut cx),
        ((400, 300, 250, 60), (400, 300, 250, 60))
    );
}
