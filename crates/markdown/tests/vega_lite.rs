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

fn chart(source: &'static str, cx: &mut TestAppContext) -> Option<gpui::Bounds<gpui::Pixels>> {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Page(source));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(320.0), px(800.0)));
    cx.run_until_parked();
    cx.debug_bounds(markdown::charts::SELECTOR)
}

#[gpui::test]
fn a_vega_lite_fence_paints_its_chart_at_the_stated_height(cx: &mut TestAppContext) {
    let bounds = chart(
        "```vega-lite 300\n{\"data\": {\"values\": [{\"a\": \"x\", \"b\": 1}]}, \"mark\": \"bar\", \"encoding\": {\"x\": {\"field\": \"a\", \"type\": \"nominal\"}, \"y\": {\"field\": \"b\", \"type\": \"quantitative\"}}}\n```\n",
        cx,
    )
    .expect("the fence paints a chart");
    assert!(bounds.size.height > px(200.0) && bounds.size.height < px(300.0));
}

#[gpui::test]
fn a_spec_it_cannot_read_keeps_its_source(cx: &mut TestAppContext) {
    assert!(chart("```vega-lite\n{\"mark\": \"bar\"\n```\n", cx).is_none());
}
