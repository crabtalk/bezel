//! An app's own kinds get what the spec's do: any node holds others, by its
//! kind or by name, and the canvas makes the app's node from nothing.

use canvas::{
    Canvas, CanvasView, Kinds, contain,
    kind::{Field, Kind, Look},
    layout,
    model::Node,
};
use gpui::{
    ClipboardItem, Entity, Focusable, IntoElement, Modifiers, MouseButton, MouseDownEvent,
    MouseUpEvent, Pixels, Point, TestAppContext, VisualTestContext, div, point, px, size,
};

/// A board holds `c`, which sits in it; `e` names `d`, which holds nothing by
/// its kind.
const DOC: &str = r#"{
  "nodes": [
    {"id":"b","type":"board","x":0,"y":0,"width":400,"height":300},
    {"id":"c","type":"card","x":40,"y":40,"width":120,"height":60,"label":"c"},
    {"id":"d","type":"card","x":600,"y":0,"width":120,"height":60,"label":"d"},
    {"id":"e","type":"card","x":600,"y":200,"width":120,"height":60,"label":"e","container":"d"}
  ]
}"#;

fn card() -> Node {
    Node {
        kind: "card".into(),
        width: 120,
        height: 60,
        ..Node::default()
    }
}

fn open(cx: &mut TestAppContext) -> (Entity<CanvasView>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        let blank =
            |_: &Node, _: Look, _: &mut gpui::Window, _: &mut gpui::App| div().into_any_element();
        let kinds = Kinds::new()
            .with("board", Kind::new(blank).holds())
            .with(
                "card",
                Kind::new(blank).edit(Field::new(
                    |node| node.label.clone().unwrap_or_default(),
                    |node, label| node.label = Some(label),
                )),
            )
            .with_fresh(card);
        canvas::set_kinds(cx, kinds);
        canvas::init(cx);
    });
    let doc = Canvas::parse(DOC).unwrap();
    let window = cx.add_window(move |_, cx| CanvasView::new(doc, cx).with_layout(layout::FREE));
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    cx.update(|window, cx| {
        let handle = view.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    (view, cx)
}

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

fn doc(view: &Entity<CanvasView>, cx: &mut VisualTestContext) -> Canvas {
    cx.update(|_, cx| view.read(cx).canvas().clone())
}

#[gpui::test]
fn a_kind_that_holds_carries_what_sits_in_it(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    let zoom = cx.update(|_, cx| view.read(cx).zoom());
    let from = screen(&view, (300, 250), &mut cx);
    let to = from + point(px(60.0 * zoom), px(40.0 * zoom));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());

    let canvas = doc(&view, &mut cx);
    let at = |id| {
        let node = canvas.node(id).unwrap();
        (node.x, node.y)
    };
    assert_eq!([at("b"), at("c"), at("d")], [(60, 40), (100, 80), (600, 0)]);
}

#[gpui::test]
fn a_double_click_on_nothing_makes_the_apps_node(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    let at = screen(&view, (500, 400), &mut cx);
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
    let canvas = doc(&view, &mut cx);
    let made = canvas.nodes.last().unwrap();
    assert_eq!(
        (made.kind.as_str(), made.width, made.height),
        ("card", 120, 60)
    );
}

#[gpui::test]
fn pasted_text_lands_in_the_apps_field(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    cx.update(|_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string("hello".into()));
        view.update(cx, |view, cx| view.paste(cx));
    });
    let canvas = doc(&view, &mut cx);
    let made = canvas.nodes.last().unwrap();
    assert_eq!(
        (made.kind.as_str(), made.label.as_deref()),
        ("card", Some("hello"))
    );
}

fn drag_by(
    view: &Entity<CanvasView>,
    from: (i64, i64),
    by: (f32, f32),
    cx: &mut VisualTestContext,
) {
    let zoom = cx.update(|_, cx| view.read(cx).zoom());
    let start = screen(view, from, cx);
    let end = start + point(px(by.0 * zoom), px(by.1 * zoom));
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
}

#[gpui::test]
fn a_node_naming_a_container_goes_where_it_goes(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    // `e` names `d`, which is no kind that holds, and sits outside it.
    drag_by(&view, (660, 30), (-40.0, 20.0), &mut cx);
    let canvas = doc(&view, &mut cx);
    let at = |id| {
        let node = canvas.node(id).unwrap();
        (node.x, node.y)
    };
    assert_eq!([at("d"), at("e")], [(560, 20), (560, 220)]);
    assert_eq!(contain::named(canvas.node("e").unwrap()), Some("d"));

    // Carried off on its own, outside `d`, it lets `d` go.
    drag_by(&view, (620, 250), (0.0, 100.0), &mut cx);
    let canvas = doc(&view, &mut cx);
    assert_eq!(contain::named(canvas.node("e").unwrap()), None);
}
