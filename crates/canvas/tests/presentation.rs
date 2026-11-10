use canvas::{
    Canvas, CanvasView, Change, Kinds, Options, handle,
    kind::{self, Kind, Sizing},
    layout,
    presentation::{self, Detail},
};
use gpui::{TestAppContext, VisualTestContext, div, prelude::*, px, size};
use std::{cell::Cell, rc::Rc};
use theme::TextStyle;

#[test]
fn detail_depends_on_displayed_size_and_legibility() {
    assert_eq!(Detail::at(200., 100., 16., false), Detail::Full);
    assert_eq!(Detail::at(200., 100., 8., false), Detail::Summary);
    assert_eq!(Detail::at(100., 100., 16., false), Detail::Full);
    assert_eq!(Detail::at(40., 100., 8., false), Detail::Silhouette);
    assert_eq!(Detail::at(200., 10., 8., false), Detail::Silhouette);
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
        ).with_kinds(Kinds::new().with("text", kind::text().sizing(Sizing::Fixed)))
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

#[gpui::test]
fn selected_text_keeps_its_content_rendering_after_a_small_resize(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let seen = Rc::new(Cell::new(None));
    let record = seen.clone();
    let window = cx.add_window(move |_, cx| {
        let mut text = kind::text().sizing(Sizing::Fixed).handles(handle::frame);
        let render = text.render.clone();
        text.render = Rc::new(move |node, look, window, cx| {
            record.set(Some(look.detail));
            render(node, look, window, cx)
        });
        CanvasView::new(Canvas::parse(r#"{"nodes":[{"id":"text","type":"text","x":100,"y":100,"width":360,"height":100,"text":"First paragraph.\n\nSecond paragraph."}]}"#).unwrap(), layout::FREE, cx)
            .with_kinds(Kinds::new().with("text", text))
    });
    let view = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(800.), px(600.)));
    view.update(&mut visual, |view, cx| {
        view.update_editor(cx, |editor| {
            editor.select(Some("text".into()));
            editor.set_zoom(0.5);
        })
    });
    for _ in 0..3 {
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
    }
    assert_eq!(seen.get(), Some(Detail::Full));
    view.update(&mut visual, |view, cx| {
        view.update_editor(cx, |editor| {
            let mut node = editor.canvas().node("text").unwrap().clone();
            node.width = 120;
            node.height = 40;
            editor.apply([Change::UpdateNode { node }]);
        });
    });
    for _ in 0..3 {
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
    }
    assert_eq!(seen.get(), Some(Detail::Full));
    visual.update(|_, cx| {
        let node = view.read(cx).editor().canvas().node("text").unwrap();
        assert_eq!((node.width, node.height), (120, 40));
        assert_eq!(
            node.text.as_deref(),
            Some("First paragraph.\n\nSecond paragraph.")
        );
    });
}

#[gpui::test]
fn embedded_editing_uses_canvas_typography_without_document_size_clamping(cx: &mut TestAppContext) {
    use editor::AppExt as _;
    cx.update(|cx| {
        cx.set_editor_text_size(editor::TextSize {
            min: 12.,
            max: 24.,
            step: 1.,
        });
        cx.adjust_editor_text_size(5.);
        for authored in [16., 48., 96.] {
            for zoom in [0.25, 0.5, 1., 2.] {
                let expected = presentation::text_typography(cx, authored, zoom);
                let input = cx.new(|cx| {
                    let mut input = editor::Editor::new("## Heading\n\nBody", cx);
                    input.typography = Some(expected);
                    input
                });
                let actual = input.read(cx).resolved_typography(cx);
                assert_eq!(actual.body, expected.body);
                assert_eq!(actual.h2, expected.h2);
                assert_eq!(actual.body.line_height(), expected.body.line_height());
            }
        }
    });
}

#[test]
fn annotation_sizing_grows_even_when_document_cards_have_fixed_geometry() {
    let kinds = Kinds::new().with("text", kind::text().sizing(Sizing::Fixed));
    let canvas = Canvas::parse(r#"{"nodes":[{"id":"card","type":"text","x":0,"y":0,"width":200,"height":100},{"id":"label","type":"text","x":200,"y":0,"width":200,"height":100,"textMode":"label"}]}"#).unwrap();
    assert_eq!(kinds.sizing(&canvas.nodes[0]), Sizing::Fixed);
    assert_eq!(kinds.sizing(&canvas.nodes[1]), Sizing::Grows);
    let handles = handle::frame(&canvas.nodes[1]);
    assert_eq!(
        handles
            .iter()
            .filter(|handle| handle.role == canvas::handle::Role::Resize)
            .count(),
        6
    );
}
