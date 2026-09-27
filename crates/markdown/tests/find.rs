//! A range scrolled into view and a picture as a control, under gpui's own
//! harness — both are answers only a real layout gives.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    App, Context, Modifiers, MouseButton, Render, ScrollHandle, TestAppContext, VisualTestContext,
    Window, div, point, prelude::*, px, size,
};
use markdown::{
    BlockLayouts, Cursor, Doc, Editing, OnImage, Part, Selection, parse, render_source, render_with,
};

const WIDTH: f32 = 320.0;
const HEIGHT: f32 = 400.0;

struct Page {
    doc: Doc,
    source: Option<String>,
    layouts: BlockLayouts,
    scroll: ScrollHandle,
    /// Every block the image hook was called with, in order.
    clicked: Rc<RefCell<Vec<usize>>>,
    hooked: bool,
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let clicked = self.clicked.clone();
        let image = self.hooked.then(|| {
            Rc::new(move |ix: usize, _: &mut Window, _: &mut App| clicked.borrow_mut().push(ix))
                as OnImage
        });
        let editing = Editing {
            layouts: Some(&self.layouts),
            scroll: Some(&self.scroll),
            image,
            ..Editing::default()
        };
        let body = match &self.source {
            Some(source) => render_source(source, editing, cx),
            None => render_with(&self.doc, editing, window, cx),
        };
        div()
            .id("page")
            .w(px(WIDTH))
            .h(px(HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(body)
    }
}

fn open(
    source: &str,
    as_source: bool,
    hooked: bool,
    cx: &mut TestAppContext,
) -> (gpui::Entity<Page>, VisualTestContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Page {
        doc: parse(source),
        source: as_source.then(|| source.to_string()),
        layouts: BlockLayouts::default(),
        scroll: ScrollHandle::new(),
        clicked: Rc::new(RefCell::new(Vec::new())),
        hooked,
    });
    let page = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(WIDTH), px(HEIGHT)));
    settle(&mut visual);
    (page, visual)
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..8 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

fn paragraphs(count: usize) -> String {
    (0..count)
        .map(|ix| format!("paragraph {ix}"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn shown(page: &gpui::Entity<Page>, range: Selection, cx: &mut VisualTestContext) -> bool {
    let rows = cx.update(|_, cx| page.read(cx).layouts.rects(range));
    rows.first()
        .is_some_and(|row| row.top() >= px(0.0) && row.bottom() <= px(HEIGHT))
}

#[gpui::test]
fn a_reveal_scrolls_a_block_never_built_into_view(cx: &mut TestAppContext) {
    let (page, mut cx) = open(&paragraphs(300), false, false, cx);
    let range = Selection {
        anchor: Cursor::new(250, Part::Body, 0),
        head: Cursor::new(250, Part::Body, 9),
    };
    assert!(!shown(&page, range, &mut cx), "the block starts off-screen");

    cx.update(|_, cx| page.read(cx).layouts.reveal(range));
    settle(&mut cx);

    assert!(shown(&page, range, &mut cx), "the range is on screen");
}

#[gpui::test]
fn a_reveal_scrolls_back_up(cx: &mut TestAppContext) {
    let (page, mut cx) = open(&paragraphs(300), false, false, cx);
    let (low, high) = (
        Selection::new(
            Cursor::new(280, Part::Body, 0),
            Cursor::new(280, Part::Body, 9),
        ),
        Selection {
            anchor: Cursor::new(3, Part::Body, 0),
            head: Cursor::new(3, Part::Body, 9),
        },
    );
    cx.update(|_, cx| page.read(cx).layouts.reveal(low));
    settle(&mut cx);
    cx.update(|_, cx| page.read(cx).layouts.reveal(high));
    settle(&mut cx);

    assert!(shown(&page, high, &mut cx), "the range is on screen");
}

#[gpui::test]
fn a_reveal_in_source_finds_its_line(cx: &mut TestAppContext) {
    let source = paragraphs(300);
    let offset = source.find("paragraph 260").unwrap();
    let (page, mut cx) = open(&source, true, false, cx);
    let range = Selection {
        anchor: Cursor::new(0, Part::Code, offset),
        head: Cursor::new(0, Part::Code, offset + 13),
    };

    cx.update(|_, cx| page.read(cx).layouts.reveal(range));
    settle(&mut cx);

    assert!(shown(&page, range, &mut cx), "the line is on screen");
}

fn picture(page: &gpui::Entity<Page>, cx: &mut VisualTestContext) -> gpui::Bounds<gpui::Pixels> {
    cx.update(|_, cx| page.read(cx).layouts.picture_bounds(1))
        .expect("the picture painted")
}

fn clicked(page: &gpui::Entity<Page>, cx: &mut VisualTestContext) -> Vec<usize> {
    cx.update(|_, cx| page.read(cx).clicked.borrow().clone())
}

/// A 1×1 PNG, which a stated width scales to a square.
const PIXEL: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 218, 99, 100, 96, 248, 95, 15, 0, 2,
    135, 1, 128, 235, 71, 186, 146, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

/// A picture between two paragraphs, as block 1.
fn picture_doc() -> String {
    let path = std::env::temp_dir().join(format!("bezel-find-{}.png", std::process::id()));
    std::fs::write(&path, PIXEL).unwrap();
    format!("before\n\n![a picture|200]({})\n\nafter", path.display())
}

#[gpui::test]
fn a_click_on_a_picture_calls_the_hook(cx: &mut TestAppContext) {
    let (page, mut cx) = open(&picture_doc(), false, true, cx);

    let center = picture(&page, &mut cx).center();
    cx.simulate_click(center, Modifiers::default());

    assert_eq!(clicked(&page, &mut cx), vec![1]);
}

#[gpui::test]
fn a_drag_over_a_picture_does_not(cx: &mut TestAppContext) {
    let (page, mut cx) = open(&picture_doc(), false, true, cx);

    let from = picture(&page, &mut cx).center();
    let to = from + point(px(20.0), px(0.0));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::default());

    assert!(clicked(&page, &mut cx).is_empty());
}

#[gpui::test]
fn a_picture_without_a_hook_takes_nothing(cx: &mut TestAppContext) {
    let (page, mut cx) = open(&picture_doc(), false, false, cx);

    let center = picture(&page, &mut cx).center();
    cx.simulate_click(center, Modifiers::default());

    assert!(clicked(&page, &mut cx).is_empty());
}
