//! What the canvas is tuned by, and the overlays a node wears.

use std::{cell::Cell, rc::Rc};

use canvas::{
    Canvas, CanvasEditor, CanvasView, Options, Overlays,
    layout::{self, Arrow},
};
use gpui::{IntoElement, TestAppContext, VisualTestContext, div, px, size};

const ONE: &str = r#"{
  "nodes": [{"id":"a","type":"link","x":0,"y":0,"width":200,"height":40,"url":"a"}]
}"#;

fn editor(options: Options) -> CanvasEditor {
    CanvasEditor::new(Canvas::parse(ONE).unwrap(), layout::FREE).with_options(options)
}

#[test]
fn zoom_stops_where_the_options_say() {
    let mut editor = editor(Options {
        max_zoom: 8.0,
        zoom_step: 2.0,
        ..Options::default()
    });
    editor.set_viewport(size(800.0, 600.0));
    for _ in 0..10 {
        editor.zoom_in();
    }
    assert_eq!(editor.zoom(), 8.0);
    for _ in 0..20 {
        editor.zoom_out();
    }
    assert_eq!(editor.zoom(), editor.options().min_zoom);
}

#[test]
fn a_nudge_steps_by_what_the_options_say() {
    let mut editor = editor(Options {
        nudge: 25,
        ..Options::default()
    });
    editor.select(Some("a".into()));
    editor.nudge(Arrow::Right);
    assert_eq!(editor.canvas().node("a").unwrap().x, 25);
}

#[test]
fn the_history_it_keeps_is_the_options() {
    let mut editor = editor(Options {
        history: 2,
        ..Options::default()
    });
    editor.select(Some("a".into()));
    for _ in 0..5 {
        editor.nudge(Arrow::Right);
    }
    // Only the last two steps are there to take back.
    let mut taken = 0;
    while editor.undo() {
        taken += 1;
    }
    assert_eq!(taken, 2);
}

#[gpui::test]
fn a_replaced_overlay_is_what_paints(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let painted = Rc::new(Cell::new(false));
    let overlays = Overlays {
        ring: {
            let painted = painted.clone();
            Rc::new(move |_, _, _| {
                painted.set(true);
                div().into_any_element()
            })
        },
        ..Overlays::new()
    };
    let doc = Canvas::parse(ONE).unwrap();
    let window =
        cx.add_window(move |_, cx| CanvasView::new(doc, layout::FREE, cx).with_overlays(overlays));
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    // Nothing is picked, so no ring has been asked for yet.
    assert!(!painted.get());

    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.update_editor(cx, |editor| editor.select(Some("a".into())))
        })
    });
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    assert!(painted.get(), "the app's ring never painted");
}
