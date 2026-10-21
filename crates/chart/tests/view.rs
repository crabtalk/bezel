//! Each mark draws, and redraws under a pointer, in a window.

use chart::{
    Channel, Chart, Data, Encode, Encoding, Mark, Properties, Shape,
    model::{Align, Baseline, Cap, Dir, Extent, Interpolate, Join, LineBreak, Overlay},
};
use gpui::{
    App, Context, FontStyle, FontWeight, IntoElement, Modifiers, Render, RenderImage,
    TestAppContext, VisualTestContext, Window, black, div, point, prelude::*, px, size,
};
use std::{cell::RefCell, sync::Arc};

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
            .x(Channel::quantitative("x"))
            .y(Channel::quantitative("sales"))
            .color(Channel::nominal("region"))
    };
    vec![
        Chart::bar(data.clone())
            .x(Channel::nominal("month"))
            .y(Channel::quantitative("sales"))
            .color(Channel::nominal("region"))
            .title("Sales"),
        along(Chart::line(data.clone())),
        Chart::line(data.clone())
            .y(Channel::nominal("month"))
            .x(Channel::quantitative("sales"))
            .color(Channel::nominal("region")),
        along(Chart::area(data.clone())),
        Chart::arc(data.clone())
            .theta(Channel::quantitative("x"))
            .color(Channel::nominal("region"))
            .x(Channel::nominal("month"))
            .y(Channel::quantitative("sales"))
            .radius(Encode::Value(30.0)),
        along(Chart::point(data.clone())),
        Chart::bar(data.clone())
            .x(Channel::nominal("month"))
            .y(Channel::quantitative("sales"))
            .layer(
                Chart::line(data.clone())
                    .x(Channel::nominal("month"))
                    .y(Channel::quantitative("x")),
            ),
        Chart::arc(data.clone())
            .theta(Channel::quantitative("x"))
            .color(Channel::nominal("month")),
        Chart::bar(data.clone())
            .x(Channel::nominal("month"))
            .y(Channel::quantitative("sales"))
            .x_offset(Channel::nominal("region"))
            .color(Channel::nominal("region"))
            .stroke(Encode::Value(black()))
            .href(Encode::Value("https://example.com/bar".into())),
        along(Chart::line(data.clone()))
            .stroke_dash(Channel::nominal("region"))
            .stroke_width(Encode::Value(3.0)),
        along(Chart::point(data.clone()))
            .size(Channel::quantitative("sales"))
            .shape(Channel::nominal("month"))
            .angle(Channel::quantitative("x"))
            .fill_opacity(Encode::Value(0.5))
            .stroke(Channel::nominal("month")),
        Chart::arc(data.clone())
            .theta(Channel::quantitative("x"))
            .color(Channel::nominal("month"))
            .radius(Channel::quantitative("x"))
            .radius2(Encode::Value(20.0))
            .shape(Encode::Value(Shape::Diamond))
            .properties(Properties {
                pad_angle: 0.05,
                corner_radius: [Some(4.0); 4],
                ..Properties::default()
            })
            .layer(
                Chart::new(Mark::Text, data.clone())
                    .theta(Channel::quantitative("x"))
                    .color(Channel::nominal("month"))
                    .text(Channel::nominal("month"))
                    .radius(Encode::Value(60.0)),
            ),
        along(Chart::new(Mark::Rule, data.clone()))
            .y2(Channel::quantitative("x"))
            .stroke_dash(Encode::Value(vec![3.0, 2.0]))
            .properties(Properties {
                stroke_cap: Cap::Round,
                stroke_dash_offset: 1.0,
                ..Properties::default()
            })
            .layer(
                Chart::new(Mark::Rule, data.clone())
                    .y(Channel::datum(2.0, chart::Kind::Quantitative)),
            ),
        Chart::new(Mark::Rect, data.clone())
            .x(Channel::nominal("month"))
            .y(Channel::quantitative("x"))
            .y2(Channel::quantitative("sales"))
            .color(Channel::nominal("region"))
            .properties(Properties {
                corner_radius: [Some(3.0); 4],
                ..Properties::default()
            }),
        Chart::new(Mark::Tick, data.clone())
            .x(Channel::nominal("month"))
            .y(Channel::quantitative("sales"))
            .layer(
                Chart::new(Mark::Text, data.clone())
                    .x(Channel::nominal("month"))
                    .y(Channel::quantitative("sales"))
                    .text(Channel::quantitative("sales"))
                    .properties(Properties {
                        align: Some(Align::Left),
                        baseline: Some(Baseline::Top),
                        dx: 4.0,
                        font_weight: Some(FontWeight::BOLD),
                        font_style: FontStyle::Italic,
                        limit: 10.0,
                        ellipsis: Some("..".into()),
                        dir: Dir::Rtl,
                        line_break: Some(LineBreak::Text(".".into())),
                        ..Properties::default()
                    }),
            ),
        along(Chart::new(Mark::Trail, data.clone())).size(Channel::quantitative("x")),
        along(Chart::area(data.clone())).properties(Properties {
            interpolate: Interpolate::Monotone,
            line: Some(Box::new(Overlay::default())),
            point: Some(Box::new(Overlay {
                properties: Properties {
                    filled: Some(true),
                    ..Properties::default()
                },
                encoding: Encoding::default(),
            })),
            ..Properties::default()
        }),
        along(Chart::line(data.clone())).properties(Properties {
            interpolate: Interpolate::Step,
            stroke_join: Join::Round,
            ..Properties::default()
        }),
        along(Chart::new(Mark::Square, data.clone())).layer(
            Chart::new(Mark::Image, data)
                .x(Channel::quantitative("x"))
                .y(Channel::quantitative("sales"))
                .url(Channel::nominal("month"))
                .properties(Properties {
                    width: Some(Extent::Pixels(12.0)),
                    ..Properties::default()
                }),
        ),
    ]
}

/// A 4 by 2 image for every url.
fn image(_: &str, _: &mut Window, _: &mut App) -> Option<Arc<RenderImage>> {
    let buffer = image::RgbaImage::from_raw(4, 2, vec![255; 4 * 2 * 4])?;
    Some(Arc::new(RenderImage::new([image::Frame::new(buffer)])))
}

thread_local! {
    static CLICKED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn record(url: &str, _: &mut Window, _: &mut App) {
    CLICKED.with(|clicked| clicked.borrow_mut().push(url.to_string()));
}

#[gpui::test]
fn every_mark_draws_and_hovers(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Light, cx);
        chart::view::set_image_loader(cx, image);
    });
    let charts = charts();
    let undrawn: Vec<usize> = (0..charts.len())
        .filter(|&i| !charts[i].drawable())
        .collect();
    assert!(undrawn.is_empty(), "{undrawn:?}");
    let height = charts.len() as f32 * 200.0;
    let window = cx.add_window(|_, _| Root(charts));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(600.0), px(height)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    for y in (20..height as usize).step_by(50) {
        cx.simulate_mouse_move(point(px(300.0), px(y as f32)), None, Modifiers::none());
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
}

#[gpui::test]
fn a_click_on_a_linked_mark_reports_its_url(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Light, cx);
        chart::view::set_on_href(cx, record);
    });
    let data = Data::new()
        .text("month", ["Jan", "Feb"])
        .number("sales", [3.0, 5.0]);
    let bars = Chart::bar(data)
        .x(Channel::nominal("month"))
        .y(Channel::quantitative("sales"))
        .href(Encode::Value("https://example.com/bar".into()));
    let window = cx.add_window(|_, _| Root(vec![bars]));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(400.0), px(200.0)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    // Feb's bar reaches the top of the plot, right of centre.
    cx.simulate_click(point(px(300.0), px(150.0)), Modifiers::none());
    cx.simulate_click(point(px(2.0), px(2.0)), Modifiers::none());
    let clicked = CLICKED.with(|clicked| clicked.borrow().clone());
    assert_eq!(clicked, ["https://example.com/bar"]);
}
