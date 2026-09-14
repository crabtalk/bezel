//! A long line in a fence, under gpui's own harness — a real window and a real
//! text layout, which is the only place wrapping can be seen at all.

use gpui::{Context, Render, TestAppContext, VisualTestContext, Window, div, prelude::*, px, size};
use markdown::{BlockLayouts, Cursor, Doc, Editing, Layout, Part, parse, render_with, set_layout};

const WIDTH: f32 = 320.0;
const HEIGHT: f32 = 400.0;
/// Wider than the window at any plausible glyph advance, and one line of source.
const LINE: &str = "let a = b; let c = d; let e = f; let g = h; let i = j; let k = l;";

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

/// The row `offset` landed on, in window coordinates.
fn row(page: &gpui::Entity<Page>, offset: usize, cx: &mut VisualTestContext) -> f32 {
    cx.update(|_, cx| {
        let (point, _) = page
            .read(cx)
            .layouts
            .position(Cursor::new(0, Part::Code, offset))
            .expect("the fence recorded its line");
        f32::from(point.y)
    })
}

fn open(cx: &mut TestAppContext) -> (gpui::Entity<Page>, VisualTestContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Page {
        doc: parse(&format!("```\n{LINE}\n```")),
        layouts: BlockLayouts::default(),
    });
    let page = window.root(cx).unwrap();
    let visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(WIDTH), px(HEIGHT)));
    visual.run_until_parked();
    (page, visual)
}

#[gpui::test]
fn a_line_too_long_for_the_fence_wraps(cx: &mut TestAppContext) {
    let (page, mut cx) = open(cx);

    assert!(
        row(&page, LINE.len(), &mut cx) > row(&page, 0, &mut cx),
        "the end of the line sits on a row below its start"
    );
}

#[gpui::test]
fn the_scroller_keeps_the_line_on_one_row(cx: &mut TestAppContext) {
    cx.update(|cx| set_layout(cx, Layout { wrap_code: false }));
    let (page, mut cx) = open(cx);

    assert_eq!(
        row(&page, LINE.len(), &mut cx),
        row(&page, 0, &mut cx),
        "nothing wraps: the line runs off to the right instead"
    );
}
