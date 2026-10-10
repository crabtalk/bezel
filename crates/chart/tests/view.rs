//! Each mark draws, and redraws under a pointer, in a window.

use chart::{Chart, Data, Kind};
use gpui::{
    Context, IntoElement, Modifiers, Render, TestAppContext, VisualTestContext, Window, div, point,
    prelude::*, px, size,
};

struct Root(Vec<Chart>);

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().children(
            self.0
                .iter()
                .enumerate()
                .map(|(index, c)| chart::view(index, c.clone()).h(px(200.0))),
        )
    }
}

fn charts() -> Vec<Chart> {
    let data = Data::new()
        .text("month", ["Jan", "Feb", "Mar", "Jan", "Feb", "Mar"])
        .text("region", ["east", "east", "east", "west", "west", "west"])
        .number("x", [1.0, 2.0, 3.0, 1.0, 2.0, 3.0])
        .number("sales", [3.0, 5.0, 4.0, 2.0, -1.0, 6.0]);
    let along = |chart: Chart| {
        chart
            .x("x", Kind::Quantitative)
            .y("sales", Kind::Quantitative)
            .color("region", Kind::Nominal)
    };
    vec![
        Chart::bar(data.clone())
            .x("month", Kind::Nominal)
            .y("sales", Kind::Quantitative)
            .color("region", Kind::Nominal)
            .title("Sales"),
        along(Chart::line(data.clone())),
        along(Chart::area(data.clone())),
        along(Chart::point(data.clone())),
        Chart::arc(data)
            .theta("x", Kind::Quantitative)
            .color("month", Kind::Nominal),
    ]
}

#[gpui::test]
fn every_mark_draws_and_hovers(cx: &mut TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Light, cx));
    let window = cx.add_window(|_, _| Root(charts()));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(600.0), px(1000.0)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    for y in (20..1000).step_by(50) {
        cx.simulate_mouse_move(point(px(300.0), px(y as f32)), None, Modifiers::none());
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
}
