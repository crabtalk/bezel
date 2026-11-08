use canvas::{
    Canvas, CanvasView, Kinds, Options,
    kind::Kind,
    layout,
    presentation::{self, Detail},
};
use gpui::{TestAppContext, VisualTestContext, div, prelude::*};
use theme::TextStyle;

#[test]
fn detail_depends_on_displayed_size_and_legibility() {
    assert_eq!(Detail::at(200., 100., 16., false), Detail::Full);
    assert_eq!(Detail::at(200., 100., 8., false), Detail::Summary);
    assert_eq!(Detail::at(100., 100., 16., false), Detail::Summary);
    assert_eq!(Detail::at(40., 100., 16., false), Detail::Silhouette);
    assert_eq!(Detail::at(200., 10., 16., false), Detail::Silhouette);
}

#[test]
fn exports_and_editing_keep_full_content_at_every_size() {
    assert_eq!(Detail::at(10., 5., 2., true), Detail::Full);
}

#[gpui::test]
fn canvas_typography_scales_reading_sizes_together(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let full = presentation::typography(cx, 1.);
        let half = presentation::typography(cx, 0.5);
        let expected = 16. * TextStyle::Body.painted() / TextStyle::Body.size();
        assert!((full.body.size() - expected).abs() < 0.001);
        assert!((half.body.size() * 2. - full.body.size()).abs() < 0.001);
        assert!((half.h1.size() * 2. - full.h1.size()).abs() < 0.001);
    });
}

#[gpui::test]
fn fit_content_uses_measured_height_and_keeps_width_with_one_undo(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
    });
    let window = cx.add_window(|_, cx| {
        CanvasView::new(
            Canvas::parse(
                r#"{"nodes":[{"id":"a","type":"custom","width":300,"height":80,"x":0,"y":0}]}"#,
            )
            .unwrap(),
            layout::FREE,
            cx,
        )
        .with_kinds(Kinds::new().with(
            "custom",
            Kind::new(|_, look, _, _| {
                (look.measure.report)(145);
                div().into_any_element()
            }),
        ))
    });
    let view = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    view.update(cx, |view, cx| {
        view.update_editor(cx, |editor| editor.select(Some("a".into())));
        assert!(view.can_fit_content());
        view.fit_to_content(cx);
        let node = view.editor().canvas().node("a").unwrap();
        assert_eq!((node.width, node.height), (300, 145));
        view.update_editor(cx, |editor| {
            assert!(editor.undo());
        });
        assert_eq!(view.editor().canvas().node("a").unwrap().height, 80);
        view.update_editor(cx, |editor| {
            editor.set_options(Options {
                read_only: true,
                ..Default::default()
            })
        });
        assert!(!view.can_fit_content());
        view.fit_to_content(cx);
        assert_eq!(view.editor().canvas().node("a").unwrap().height, 80);
    });
}

#[gpui::test]
fn fixed_text_fits_its_natural_content_without_changing_the_saved_width(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let window = cx.add_window(|_, cx| {
        CanvasView::new(
            Canvas::parse(r##"{"nodes":[{"id":"text","type":"text","width":360,"height":80,"x":0,"y":0,"text":"# A heading\n\nFirst paragraph.\n\nSecond paragraph.\n\nThird paragraph."}]}"##).unwrap(),
            layout::FREE,
            cx,
        ).with_kinds(Kinds::new().with("text", canvas::kind::text().sizing(canvas::kind::Sizing::Fixed)))
    });
    let view = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    view.update(cx, |view, cx| {
        view.update_editor(cx, |editor| editor.select(Some("text".into())));
        assert!(view.can_fit_content());
        view.fit_to_content(cx);
        let node = view.editor().canvas().node("text").unwrap();
        assert_eq!(node.width, 360);
        assert!(node.height > 80);
    });
}
