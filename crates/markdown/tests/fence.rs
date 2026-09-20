//! A fence's copy button, under gpui's own harness — whether it is there at
//! all is answered by pressing where it floats.

use gpui::{Context, Render, TestAppContext, VisualTestContext, Window, div, prelude::*, px, size};
use markdown::{BlockLayouts, CopyButton, Doc, Editing, parse, render_with};

const WIDTH: f32 = 320.0;
const HEIGHT: f32 = 400.0;
const SOURCE: &str = "```rust\nlet a = 1;\n```";

struct Page {
    doc: Doc,
    layouts: BlockLayouts,
    copy: CopyButton,
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(WIDTH)).child(render_with(
            &self.doc,
            Editing {
                layouts: Some(&self.layouts),
                copy: self.copy,
                ..Editing::default()
            },
            window,
            cx,
        ))
    }
}

fn open(copy: CopyButton, cx: &mut TestAppContext) -> (gpui::Entity<Page>, VisualTestContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Page {
        doc: parse(SOURCE),
        layouts: BlockLayouts::default(),
        copy,
    });
    let page = window.root(cx).unwrap();
    let visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(WIDTH), px(HEIGHT)));
    visual.run_until_parked();
    (page, visual)
}

/// Where the button floats: inset from the block's top right corner, which is
/// the band the fence opens with.
fn button(page: &gpui::Entity<Page>, cx: &mut VisualTestContext) -> gpui::Point<gpui::Pixels> {
    let block = cx
        .update(|_, cx| page.read(cx).layouts.block_bounds(0))
        .expect("the fence painted");
    gpui::point(
        block.origin.x + block.size.width - px(15.0),
        block.origin.y + px(13.0),
    )
}

#[gpui::test]
fn a_shown_button_copies_the_fence(cx: &mut TestAppContext) {
    let (page, mut visual) = open(CopyButton::Shown, cx);

    let at = button(&page, &mut visual);
    visual.simulate_click(at, gpui::Modifiers::default());
    visual.run_until_parked();

    assert_eq!(
        cx.read_from_clipboard().and_then(|item| item.text()),
        Some("let a = 1;".to_string()),
        "the button wrote the fence's text"
    );
}

#[gpui::test]
fn a_hidden_button_is_not_there(cx: &mut TestAppContext) {
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("untouched".into()));
    let (page, mut visual) = open(CopyButton::Hidden, cx);

    let at = button(&page, &mut visual);
    visual.simulate_click(at, gpui::Modifiers::default());
    visual.run_until_parked();

    assert_eq!(
        cx.read_from_clipboard().and_then(|item| item.text()),
        Some("untouched".to_string()),
        "the press landed on the band"
    );
}
