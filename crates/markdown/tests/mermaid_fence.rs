use gpui::{Context, Render, TestAppContext, VisualTestContext, Window, div, prelude::*, px, size};
use markdown::{Editing, parse, render_with};

struct Page(&'static str);

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let doc = parse(self.0);
        div()
            .w(px(320.0))
            .child(render_with(&doc, Editing::default(), window, cx))
    }
}

fn diagram(source: &'static str, cx: &mut TestAppContext) -> Option<gpui::Bounds<gpui::Pixels>> {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Page(source));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(320.0), px(600.0)));
    cx.run_until_parked();
    cx.debug_bounds(markdown::fence::mermaid::LANGUAGE)
}

#[gpui::test]
fn a_mermaid_fence_paints_its_diagram(cx: &mut TestAppContext) {
    let bounds = diagram("```mermaid\nflowchart LR\n  A[Start] --> B[End]\n```\n", cx)
        .expect("the fence paints a diagram");
    assert!(bounds.size.height > px(0.0));
}

#[gpui::test]
fn a_diagram_wider_than_its_box_opens_fitted_to_it(cx: &mut TestAppContext) {
    let source = "```mermaid\nflowchart LR\n  A[Alpha] --> B[Bravo] --> C[Charlie] --> D[Delta] --> E[Echo] --> F[Foxtrot] --> G[Golf] --> H[Hotel] --> I[India] --> J[Juliet]\n```\n";
    let frame = diagram(source, cx).expect("the fence paints a diagram");
    let drawn = cx
        .windows()
        .into_iter()
        .find_map(|window| {
            VisualTestContext::from_window(window, cx)
                .debug_bounds(markdown::fence::mermaid::DIAGRAM_SELECTOR)
        })
        .expect("the diagram is drawn");
    assert!(
        drawn.size.width <= frame.size.width,
        "{:?} fits in {:?}",
        drawn.size,
        frame.size
    );
}

fn note(source: &'static str, cx: &mut TestAppContext) -> Option<gpui::Bounds<gpui::Pixels>> {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Page(source));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(320.0), px(600.0)));
    cx.run_until_parked();
    cx.debug_bounds(markdown::fence::NOTE_SELECTOR)
}

#[gpui::test]
fn a_fence_it_cannot_read_keeps_its_source_with_the_error_under_it(cx: &mut TestAppContext) {
    let source = "```mermaid\nflowchart LR\n  A -->\n```\n";
    assert!(note(source, cx).is_some());
    assert!(diagram(source, cx).is_none());
}

#[gpui::test]
fn a_pie_paints_as_a_diagram(cx: &mut TestAppContext) {
    let bounds = diagram("```mermaid\npie\n  \"a\" : 1\n  \"b\" : 2\n```\n", cx)
        .expect("the fence paints a diagram");
    assert!(bounds.size.height > px(0.0));
}

#[gpui::test]
fn a_fence_stands_empty_until_its_diagram_is_drawn(cx: &mut TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let source = "```mermaid\nflowchart LR\n  A[Start] --> B[End]\n```\n";
    let window = cx.add_window(|_, _| Page(source));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.update(|window, _| window.refresh());
    assert!(
        cx.debug_bounds(markdown::fence::mermaid::LANGUAGE)
            .is_none()
    );
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(markdown::fence::mermaid::LANGUAGE)
            .is_some()
    );
}

struct Edited(String);

impl Render for Edited {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let doc = parse(&self.0);
        div()
            .w(px(320.0))
            .child(render_with(&doc, Editing::default(), window, cx))
    }
}

#[gpui::test]
fn an_edited_fence_shows_its_last_diagram_until_the_next_is_drawn(cx: &mut TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx
        .add_window(|_, _| Edited("```mermaid\nflowchart LR\n  A[Start] --> B[End]\n```\n".into()));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    window
        .update(&mut cx, |page, _, cx| {
            page.0 = "```mermaid\nflowchart LR\n  A[Start] --> C[Next]\n```\n".into();
            cx.notify();
        })
        .unwrap();
    cx.update(|window, _| window.refresh());
    assert!(
        cx.debug_bounds(markdown::fence::mermaid::LANGUAGE)
            .is_some()
    );
}
