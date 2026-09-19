//! The rows of a wrapped selection, under gpui's own harness — a soft wrap
//! exists only once a real text layout has been given a real width.

use gpui::{Context, Render, TestAppContext, VisualTestContext, Window, div, prelude::*, px, size};
use markdown::{BlockLayouts, Cursor, Doc, Editing, Part, Selection, parse, render_with};

const WIDTH: f32 = 320.0;
const HEIGHT: f32 = 400.0;
/// Wider than the window at any plausible glyph advance, and one paragraph.
/// The accents are two bytes each, so a byte that is not a boundary is never
/// far from wherever the wrap falls.
const LINE: &str = "été après été, où naît déjà né, à côté dû thé, fût créé, êtes près, là même";

struct Page {
    doc: Doc,
    layouts: BlockLayouts,
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(WIDTH)).child(render_with(
            &self.doc,
            Editing {
                layouts: Some(&self.layouts),
                ..Editing::default()
            },
            window,
            cx,
        ))
    }
}

fn open(cx: &mut TestAppContext) -> (gpui::Entity<Page>, VisualTestContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Page {
        doc: parse(LINE),
        layouts: BlockLayouts::default(),
    });
    let page = window.root(cx).unwrap();
    let visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(WIDTH), px(HEIGHT)));
    visual.run_until_parked();
    (page, visual)
}

#[gpui::test]
fn every_wrapped_row_is_covered_from_its_first_glyph(cx: &mut TestAppContext) {
    let (page, mut cx) = open(cx);

    let rects = cx.update(|_, cx| {
        page.read(cx).layouts.rects(Selection::new(
            Cursor::new(0, Part::Body, 0),
            Cursor::new(0, Part::Body, LINE.len()),
        ))
    });

    assert!(rects.len() > 1, "the paragraph wraps");
    let left = rects[0].origin.x;
    for (row, rect) in rects.iter().enumerate() {
        assert_eq!(
            rect.origin.x, left,
            "row {row} starts where the paragraph does, not one glyph in"
        );
    }
}
