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
    cx.debug_bounds(markdown::mermaid::LANGUAGE)
}

#[gpui::test]
fn a_mermaid_fence_paints_its_diagram(cx: &mut TestAppContext) {
    let bounds = diagram("```mermaid\nflowchart LR\n  A[Start] --> B[End]\n```\n", cx)
        .expect("the fence paints a diagram");
    assert!(bounds.size.height > px(0.0));
}

#[gpui::test]
fn a_fence_it_cannot_read_keeps_its_source(cx: &mut TestAppContext) {
    assert!(diagram("```mermaid\nflowchart LR\n  A -->\n```\n", cx).is_none());
}
