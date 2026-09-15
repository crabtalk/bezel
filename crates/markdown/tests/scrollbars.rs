use gpui::{
    Context, Modifiers, MouseButton, Render, TestAppContext, VisualTestContext, Window, div,
    prelude::*, px, size,
};
use markdown::{BlockLayouts, Cursor, Doc, Editing, Layout, Part, parse, render_with, set_layout};
use ui::scroll::{Visibility, set_visibility};

struct Page {
    doc: Doc,
    layouts: BlockLayouts,
}
impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(240.)).child(render_with(
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

fn open(source: &str, cx: &mut TestAppContext) -> (gpui::Entity<Page>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        set_layout(cx, Layout { wrap_code: false });
        set_visibility(Visibility::Always, cx);
    });
    let window = cx.add_window(|_, _| Page {
        doc: parse(source),
        layouts: BlockLayouts::default(),
    });
    let page = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(240.), px(400.)));
    visual.update(|window, _| window.refresh());
    visual.run_until_parked();
    (page, visual)
}

fn x(page: &gpui::Entity<Page>, part: Part, cx: &mut VisualTestContext) -> gpui::Pixels {
    cx.update(|_, cx| {
        page.read(cx)
            .layouts
            .position(Cursor::new(0, part, 0))
            .unwrap()
            .0
            .x
    })
}

fn drag_track(selector: &'static str, cx: &mut VisualTestContext) {
    let bounds = cx
        .debug_bounds(selector)
        .expect("overflow should show a scrollbar");
    let mut pointer = bounds.origin + gpui::point(px(8.), px(5.));
    cx.simulate_mouse_move(pointer, None, Modifiers::default());
    cx.simulate_mouse_down(pointer, MouseButton::Left, Modifiers::default());
    pointer.x += px(10.);
    cx.simulate_mouse_move(pointer, MouseButton::Left, Modifiers::default());
    pointer.x += px(100.);
    cx.simulate_mouse_move(pointer, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(pointer, MouseButton::Left, Modifiers::default());
}

#[gpui::test]
fn code_scrollbar_moves_text_and_respects_visibility(cx: &mut TestAppContext) {
    let (page, mut cx) = open(
        &format!("```\n{}\n```", "long_code_identifier ".repeat(30)),
        cx,
    );
    let before = x(&page, Part::Code, &mut cx);
    drag_track("md-code-scroll-0-track", &mut cx);
    assert!(x(&page, Part::Code, &mut cx) < before);
    cx.update(|_, cx| set_visibility(Visibility::Never, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("md-code-scroll-0-track").is_none());
}

#[gpui::test]
fn wide_table_scrollbar_moves_cells(cx: &mut TestAppContext) {
    let (page, mut cx) = open(
        "| First | Second | Third | Fourth |\n| --- | --- | --- | --- |\n| a | b | c | d |",
        cx,
    );
    let part = Part::Cell { row: 0, column: 3 };
    let before = x(&page, part, &mut cx);
    drag_track("md-table-scroll-0-track", &mut cx);
    assert!(
        x(&page, part, &mut cx) < before,
        "before={before:?}, after={:?}, track={:?}",
        x(&page, part, &mut cx),
        cx.debug_bounds("md-table-scroll-0-track")
    );
}

#[gpui::test]
fn short_or_wrapped_code_has_no_horizontal_bar(cx: &mut TestAppContext) {
    let (page, mut cx) = open("```\nshort\n```", cx);
    assert!(cx.debug_bounds("md-code-scroll-0-track").is_none());
    cx.update(|_, cx| {
        set_layout(cx, Layout { wrap_code: true });
        page.update(cx, |page, cx| {
            page.doc = parse(&format!("```\n{}\n```", "long_code_identifier ".repeat(30)));
            cx.notify();
        });
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("md-code-scroll-0-track").is_none());
}
