use canvas::{Canvas, CanvasEditor, CanvasView, Kinds, export::Area, kind::Kind, layout};
use gpui::{AppContext as _, TestAppContext, div, point, prelude::*, size};

const SOURCE: &str = r#"{"nodes":[{"id":"a","type":"text","x":-100,"y":40,"width":200,"height":100,"text":"Note"},{"id":"b","type":"text","x":400,"y":90,"width":100,"height":100,"text":"Other"}],"edges":[{"id":"ab","fromNode":"a","toNode":"b","points":[[-200,-40],[700,250]]}]}"#;

#[test]
fn full_export_includes_negative_coordinates_and_edge_routes() {
    let editor = CanvasEditor::new(Canvas::parse(SOURCE).unwrap(), layout::FREE);
    let frame = Area::Canvas.frame(&editor).unwrap();
    assert_eq!(frame.size, size(964., 354.));
    assert_eq!(frame.pan, point(232., 72.));
    assert_eq!(frame.zoom, 1.);
}

#[test]
fn viewport_export_keeps_the_current_zoom_pan_and_size() {
    let mut editor = CanvasEditor::new(Canvas::parse(SOURCE).unwrap(), layout::FREE);
    assert!(Area::Viewport.frame(&editor).is_err());
    editor.set_viewport(size(800., 600.));
    editor.set_zoom(2.);
    editor.pan_by(15., -30.);
    let frame = Area::Viewport.frame(&editor).unwrap();
    assert_eq!(frame.size, size(800., 600.));
    assert_eq!(frame.pan, editor.pan());
    assert_eq!(frame.zoom, 2.);
}

#[gpui::test]
fn custom_native_content_is_refused_before_opening_an_export_window(cx: &mut TestAppContext) {
    let view = cx.update(|cx| {
        cx.new(|cx| {
            let mut document = Canvas::parse(SOURCE).unwrap();
            document.nodes[0].kind = "native".into();
            CanvasView::new(document, layout::FREE, cx).with_kinds(
                Kinds::new().with("native", Kind::new(|_, _, _, _| div().into_any_element())),
            )
        })
    });
    let result = view.update(cx, |view, cx| view.export_image(Area::Canvas, 1., cx));
    assert!(
        result
            .unwrap_err()
            .contains("does not support image export")
    );
    assert!(cx.windows().is_empty());
}

#[gpui::test]
fn unsupported_nodes_outside_the_viewport_do_not_block_its_export(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let view = cx.update(|cx| {
        cx.new(|cx| {
            let mut document = Canvas::parse(SOURCE).unwrap();
            document.nodes[0].kind = "native".into();
            let mut view = CanvasView::new(document, layout::FREE, cx);
            view.update_editor(cx, |editor| {
                editor.set_viewport(size(200., 100.));
                editor.pan_by(-10000., 0.);
            });
            view
        })
    });
    let task = view
        .update(cx, |view, cx| view.export_image(Area::Viewport, 1., cx))
        .unwrap();
    drop(task);
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
}
