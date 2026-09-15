//! Finding one's way: fit, zoom to the selection, a selection kept in view,
//! and the part in view read and moved.

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
        let (x, y, w, h) = view.visible().unwrap();
        let node = view.canvas().node(id).unwrap();
        let (left, top) = (node.x as f32, node.y as f32);
        left >= x
            && top >= y
            && left + node.width as f32 <= x + w
            && top + node.height as f32 <= y + h
    })
}

#[gpui::test]
fn fit_shows_everything(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    assert!(!in_view(&view, "b", &mut cx));
    cx.update(|_, cx| view.update(cx, |view, cx| view.fit(cx)));
    assert!(cx.update(|_, cx| view.read(cx).zoom()) < 1.0);
    for id in ["a", "b", "c"] {
        assert!(in_view(&view, id, &mut cx), "{id} is out of view");
    }
}

#[gpui::test]
fn zoom_to_selection_centres_it(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.select(Some("c".into()), cx);
            view.zoom_to_selection(cx);
        })
    });
    assert!(in_view(&view, "c", &mut cx));
    let (x, y, w, h) = cx.update(|_, cx| view.read(cx).visible().unwrap());
    assert!((x + w / 2.0 - 1600.0).abs() < 1.0 && (y + h / 2.0 - 1220.0).abs() < 1.0);
}

#[gpui::test]
fn an_arrow_brings_what_it_picks_into_view(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    cx.update(|_, cx| view.update(cx, |view, cx| view.select(Some("a".into()), cx)));
    cx.simulate_keystrokes("right");
    draw(&mut cx);
    let picked = cx.update(|_, cx| view.read(cx).selected().map(str::to_owned));
    assert_eq!(picked.as_deref(), Some("b"));
    assert!(in_view(&view, "b", &mut cx));
}

#[gpui::test]
fn center_on_puts_a_point_in_the_middle(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    cx.update(|_, cx| view.update(cx, |view, cx| view.center_on((1500.0, 600.0), cx)));
    let (x, y, w, h) = cx.update(|_, cx| view.read(cx).visible().unwrap());
    assert_eq!((x + w / 2.0, y + h / 2.0), (1500.0, 600.0));
}
