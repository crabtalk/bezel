//! A Vega-Lite bar chart compiled, run to its scenegraph and painted.

use gpui::{
    Context, IntoElement, Render, TestAppContext, VisualTestContext, Window, div, prelude::*, px,
    size,
};
use vega::{Drawing, View};
use vega_lite::spec::TopLevel;

/// Vega-Lite's `bar.vl.json`.
const BAR: &str = r#"{
  "data": {"values": [
    {"a": "A", "b": 28}, {"a": "B", "b": 55}, {"a": "C", "b": 43},
    {"a": "D", "b": 91}, {"a": "E", "b": 81}, {"a": "F", "b": 53},
    {"a": "G", "b": 19}, {"a": "H", "b": 87}, {"a": "I", "b": 52}
  ]},
  "mark": "bar",
  "encoding": {
    "x": {"field": "a", "type": "nominal", "axis": {"labelAngle": 0}},
    "y": {"field": "b", "type": "quantitative"}
  }
}"#;

fn drawing() -> Drawing {
    let spec: TopLevel = serde_json::from_str(BAR).expect("spec");
    let vega = vega_lite::compile(&spec).expect("compiles");
    let view = View::new(vega, &|_| None).expect("a view");
    view.drawing().expect("draws")
}

struct Root(Drawing);

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(chart::scene("bar", self.0.clone()))
    }
}

#[test]
fn a_bar_chart_takes_the_size_vega_gives_it() {
    let drawing = drawing();
    assert_eq!((drawing.width, drawing.height), (226.0, 337.0));
    assert_eq!(drawing.origin, [46.0, 5.0]);
    assert_eq!(drawing.padding, [5.0; 4]);
    assert_eq!(drawing.background.as_deref(), Some("white"));
}

#[gpui::test]
fn a_bar_chart_paints(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| Root(drawing()));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(400.0), px(400.0)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
}
