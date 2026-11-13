use canvas::{Canvas, CanvasView, Kinds, change::Change, kind::Kind, layout};
use gpui::{TestAppContext, VisualTestContext, div, prelude::*, px, rgb, size};
use std::{cell::Cell, rc::Rc};

#[gpui::test]
fn retained_fixed_bodies_skip_camera_renders_and_invalidate_on_zoom(cx: &mut TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let renders = Rc::new(Cell::new(0));
    let window = cx.add_window({
        let renders = renders.clone();
        move |_, cx| CanvasView::new(
            Canvas::parse(r#"{"nodes":[{"id":"a","type":"retained","x":0,"y":0,"width":100,"height":80}]}"#).unwrap(),
            layout::FREE, cx,
        ).with_kinds(Kinds::new().with("retained", Kind::new(move |_, _, _, _| {
            renders.set(renders.get() + 1);
            div().size_full().bg(rgb(0xff0000)).into_any_element()
        }).retained()))
    });
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.), px(600.)));
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    let before = renders.get();
    view.update(&mut cx, |view, cx| {
        view.update_editor(cx, |editor| editor.pan_by(20., 0.))
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(renders.get(), before);
    view.update(&mut cx, |view, cx| {
        view.update_editor(cx, |editor| editor.set_zoom(0.5))
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(renders.get() > before);
    let before = renders.get();
    view.update(&mut cx, |view, cx| {
        view.update_editor(cx, |editor| {
            let mut node = editor.canvas().node("a").unwrap().clone();
            node.label = Some("Updated content".into());
            editor.submit([Change::UpdateNode { node }]);
        });
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(renders.get(), before + 1);
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(renders.get(), before + 1);
}
