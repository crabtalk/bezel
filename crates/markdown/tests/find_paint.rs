use gpui::{Context, Hsla, Render, TestAppContext, VisualTestContext, Window, div, prelude::*};
use markdown::{Annotation, AppExt as _, BlockLayouts, Cursor, Editing, Part, Selection};
use theme::{Appearance, Theme};

struct Page {
    source: bool,
    virtualized: bool,
    layouts: BlockLayouts,
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let part = if self.source { Part::Code } else { Part::Body };
        let range =
            |start, end| Selection::new(Cursor::new(0, part, start), Cursor::new(0, part, end));
        let annotations = [
            (range(0, 3), Annotation::Match),
            (range(4, 7), Annotation::Current),
            (
                range(8, 11),
                Annotation::Highlight(markdown::HighlightColor::Yellow),
            ),
        ];
        let editing = Editing {
            annotations: &annotations,
            layouts: self.virtualized.then_some(&self.layouts),
            ..Editing::default()
        };
        let body = if self.source {
            markdown::render_source("one two sun", editing, cx)
        } else {
            markdown::render_with(&markdown::parse("one two sun"), editing, window, cx)
        };
        div().size_full().child(body)
    }
}

fn custom(theme: &Theme) -> (Hsla, Hsla) {
    let color = markdown::highlight_solid(markdown::HighlightColor::Pink, theme);
    (color.opacity(0.22), color.opacity(0.48))
}

fn assert_washes(cx: &mut VisualTestContext, expected: (Hsla, Hsla)) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.update(|window, cx| {
        let quads = window.painted_quads();
        for wash in [
            expected.0,
            expected.1,
            markdown::default_highlight(markdown::HighlightColor::Yellow, Theme::of(cx)),
        ] {
            assert!(
                quads.iter().any(|quad| quad.background == wash.into()),
                "missing wash {wash:?}"
            );
        }
        let matched = quads
            .iter()
            .find(|quad| quad.background == expected.0.into())
            .unwrap();
        let current = quads
            .iter()
            .find(|quad| quad.background == expected.1.into())
            .unwrap();
        assert!(matched.bounds.left() < current.bounds.left());
    });
}

#[gpui::test]
fn find_washes_reach_every_render_path(cx: &mut TestAppContext) {
    for appearance in [Appearance::Light, Appearance::Dark] {
        for source in [false, true] {
            for virtualized in [false, true] {
                cx.update(|cx| Theme::install(appearance, cx));
                let window = cx.add_window(|_, _| Page {
                    source,
                    virtualized,
                    layouts: BlockLayouts::default(),
                });
                let mut visual = VisualTestContext::from_window(window.into(), cx);
                let theme = visual.update(|_, cx| Theme::of(cx).clone());
                let defaults = (theme.accent.opacity(0.22), theme.accent.opacity(0.48));
                assert_eq!(markdown::default_find(&theme), defaults);
                assert!(defaults.1.a > defaults.0.a);
                assert_washes(&mut visual, defaults);
                visual.update(|_, cx| cx.set_find_paint(custom));
                assert_washes(&mut visual, custom(&theme));
                visual.update(|_, cx| cx.set_find_paint(markdown::default_find));
                assert_washes(&mut visual, defaults);
            }
        }
    }
}
