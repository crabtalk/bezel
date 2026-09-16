//! Gestures are tools an app drops, reorders or writes its own of.

use canvas::{
    Canvas, CanvasView, layout,
    tool::{self, Hand, Hit, Pointer, Tool},
};
use gpui::{
    Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px,
    size,
};

const PAIR: &str = r#"{
  "nodes": [
    {"id":"a","type":"link","x":0,"y":0,"width":200,"height":40,"url":"a"},
    {"id":"b","type":"link","x":400,"y":300,"width":200,"height":40,"url":"b"}
  ]
}"#;

/// Selects every node the moment the background is pressed, so a press it
/// takes never reaches the tools after it.
struct SelectAll;

impl Tool for SelectAll {
    fn press(&mut self, pointer: &Pointer, hand: &mut Hand) -> bool {
        if pointer.hit != Hit::Nothing {
            return false;
        }
        hand.editor.select_all();
        true
    }
}

fn open(
    tools: Vec<Box<dyn Tool>>,
    cx: &mut TestAppContext,
) -> (Entity<CanvasView>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let doc = Canvas::parse(PAIR).unwrap();
    let window =
        cx.add_window(move |_, cx| CanvasView::new(doc, layout::FREE, cx).with_tools(tools));
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    (view, cx)
}

/// A point on empty canvas, inside the view, in window coordinates.
fn empty(view: &Entity<CanvasView>, cx: &mut VisualTestContext) -> Point<Pixels> {
    cx.update(|_, cx| {
        let view = view.read(cx);
        let (pan, zoom) = (view.editor().pan(), view.editor().zoom());
        view.bounds().unwrap().origin + point(px(pan.x + 100.0 * zoom), px(pan.y + 250.0 * zoom))
    })
}

fn drag(from: Point<Pixels>, by: (f32, f32), cx: &mut VisualTestContext) {
    let to = from + point(px(by.0), px(by.1));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
}

#[gpui::test]
fn without_the_pan_tool_the_background_does_not_pan(cx: &mut TestAppContext) {
    // Every default but the one that pans.
    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(tool::Connect::default()),
        Box::new(tool::Resize::default()),
        Box::new(tool::PickEdge),
        Box::new(tool::Marquee::default()),
        Box::new(tool::Create),
        Box::new(tool::Select::default()),
    ];
    let (view, mut cx) = open(tools, cx);
    let before = cx.update(|_, cx| view.read(cx).editor().pan());
    drag(empty(&view, &mut cx), (40.0, -20.0), &mut cx);
    let after = cx.update(|_, cx| view.read(cx).editor().pan());
    assert_eq!((after.x, after.y), (before.x, before.y));
}

#[gpui::test]
fn an_apps_tool_takes_a_press_before_the_defaults(cx: &mut TestAppContext) {
    let mut tools: Vec<Box<dyn Tool>> = vec![Box::new(SelectAll)];
    tools.extend(tool::defaults());
    let (view, mut cx) = open(tools, cx);
    let at = empty(&view, &mut cx);
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    // The default press on nothing would have selected none.
    let selected = cx.update(|_, cx| {
        view.read(cx)
            .editor()
            .selected_nodes()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    });
    assert_eq!(selected, ["a", "b"]);
}
