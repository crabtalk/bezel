//! A text node, which grows with its content, pulled taller by its corner.

use canvas::{Canvas, CanvasView, kind, layout};
use gpui::{
    Entity, Focusable, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px, size,
};

const NOTE: &str = r#"{
  "nodes": [{"id":"t","type":"text","x":0,"y":0,"width":200,"height":40,"text":"note"}]
}"#;

fn draw(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
}

fn note(view: &Entity<CanvasView>, cx: &mut VisualTestContext) -> canvas::model::Node {
    cx.update(|_, cx| view.read(cx).editor().canvas().node("t").cloned().unwrap())
}

#[gpui::test]
fn a_growing_node_pulls_taller_and_undo_lets_it_go(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let window =
        cx.add_window(|_, cx| CanvasView::new(Canvas::parse(NOTE).unwrap(), layout::FREE, cx));
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    draw(&mut cx);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.update_editor(cx, |editor| editor.select(Some("t".into())))
        });
        let handle = view.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    draw(&mut cx);

    let measured = note(&view, &mut cx).height;
    let (corner, zoom) = cx.update(|_, cx| {
        let view = view.read(cx);
        let (bounds, pan, zoom) = (
            view.bounds().unwrap(),
            view.editor().pan(),
            view.editor().zoom(),
        );
        let corner =
            bounds.origin + point(px(pan.x + 200.0 * zoom), px(pan.y + measured as f32 * zoom));
        (corner, zoom)
    });
    let to = corner + point(px(40.0 * zoom), px(80.0 * zoom));
    cx.simulate_mouse_down(corner, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    draw(&mut cx);

    let pulled = note(&view, &mut cx);
    assert_eq!((pulled.width, pulled.height), (240, measured + 80));
    assert_eq!(kind::min_height(&pulled), Some(measured + 80));

    let platform = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.simulate_keystrokes(&format!("{platform}-z"));
    draw(&mut cx);
    let back = note(&view, &mut cx);
    assert_eq!((back.width, back.height), (200, measured));
    assert_eq!(kind::min_height(&back), None);
}
