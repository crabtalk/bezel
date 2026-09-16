//! A selection an arrow picks is brought into view. Fit, zoom to the selection
//! and the part in view are in `editor.rs`.

use canvas::{Canvas, CanvasView, layout};
use gpui::{Entity, Focusable, TestAppContext, VisualTestContext, px, size};

/// Three far apart, on a free canvas.
const WIDE: &str = r#"{
  "nodes": [
    {"id":"a","type":"link","x":0,"y":0,"width":200,"height":40,"url":"a"},
    {"id":"b","type":"link","x":3000,"y":0,"width":200,"height":40,"url":"b"},
    {"id":"c","type":"link","x":1500,"y":1200,"width":200,"height":40,"url":"c"}
  ]
}"#;

fn open(cx: &mut TestAppContext) -> (Entity<CanvasView>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let doc = Canvas::parse(WIDE).unwrap();
    let window = cx.add_window(move |_, cx| CanvasView::new(doc, cx).with_layout(layout::FREE));
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    draw(&mut cx);
    cx.update(|window, cx| {
        let handle = view.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    (view, cx)
}

fn draw(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
}

/// Whether node `id` lies wholly in view.
fn in_view(view: &Entity<CanvasView>, id: &str, cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| {
        let view = view.read(cx);
        let (x, y, w, h) = view.editor().visible().unwrap();
        let node = view.editor().canvas().node(id).unwrap();
        let (left, top) = (node.x as f32, node.y as f32);
        left >= x
            && top >= y
            && left + node.width as f32 <= x + w
            && top + node.height as f32 <= y + h
    })
}

#[gpui::test]
fn an_arrow_brings_what_it_picks_into_view(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.update_editor(cx, |editor| editor.select(Some("a".into())))
        })
    });
    cx.simulate_keystrokes("right");
    draw(&mut cx);
    let picked = cx.update(|_, cx| view.read(cx).editor().selected().map(str::to_owned));
    assert_eq!(picked.as_deref(), Some("b"));
    assert!(in_view(&view, "b", &mut cx));
}
