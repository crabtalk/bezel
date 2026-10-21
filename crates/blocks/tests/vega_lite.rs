use gpui::{Context, Render, TestAppContext, VisualTestContext, Window, div, prelude::*, px, size};

const SPEC: &str = r#"{"data": {"values": [{"a": "x", "b": 1}]}, "mark": "bar",
  "encoding": {"x": {"field": "a", "type": "nominal"}, "y": {"field": "b", "type": "quantitative"}}}"#;

struct Page(&'static str, Option<u32>);

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(320.0)).children(blocks::render(
            blocks::vega_lite::LANGUAGE,
            self.0,
            self.1,
            window,
            cx,
        ))
    }
}

fn block(
    code: &'static str,
    height: Option<u32>,
    cx: &mut TestAppContext,
) -> Option<gpui::Bounds<gpui::Pixels>> {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(move |_, _| Page(code, height));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(320.0), px(800.0)));
    cx.run_until_parked();
    cx.debug_bounds(blocks::vega_lite::LANGUAGE)
}

#[gpui::test]
fn a_spec_paints_at_the_fences_height(cx: &mut TestAppContext) {
    let bounds = block(SPEC, Some(300), cx).expect("the block paints");
    assert_eq!(bounds.size.height, px(300.0));
}

#[gpui::test]
fn a_spec_it_cannot_read_is_left_to_its_source(cx: &mut TestAppContext) {
    assert!(block("{\"mark\": \"bar\"", None, cx).is_none());
}

const URL_SPEC: &str = r#"{"data": {"url": "data/a.csv"}, "mark": "bar",
  "encoding": {"x": {"field": "a", "type": "nominal"}, "y": {"field": "b", "type": "quantitative"}}}"#;

#[gpui::test]
fn a_spec_with_a_url_waits_for_the_loader(cx: &mut TestAppContext) {
    assert!(block(URL_SPEC, None, cx).is_none());
    cx.update(|cx| blocks::vega_lite::set_loader(cx, |_, _| None));
    assert!(block(URL_SPEC, None, cx).is_none());
    cx.update(|cx| {
        blocks::vega_lite::set_loader(cx, |url, _| {
            (url == "data/a.csv").then(|| "a,b\nx,1\ny,2".into())
        })
    });
    assert!(block(URL_SPEC, None, cx).is_some());
}
