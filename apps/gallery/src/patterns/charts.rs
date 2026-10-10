//! Charts — `chart::view` over columnar `chart::Data`, one per mark, a long
//! series, and a live one.
//!
//! Each chart's `Data` lives on this page and is cloned into the view every
//! frame, which keeps its generation and so its layout. The live chart builds
//! new `Data` each tick, and is the only one that lays out again.

use std::time::Duration;

use chart::{Channel, Chart, Data};
use gpui::{
    Context, Render, ScrollHandle, SharedString, Subscription, Task, Window, div, prelude::*, px,
};
use motion::Painter;
use theme::{TextStyle, Theme, Typeset};
use ui::scroll::{self, Axes, TransientState};

use crate::{hint, stack};

const TICK: Duration = Duration::from_millis(100);
/// Points the live chart keeps.
const LIVE: usize = 300;
/// Points in the long series.
const LONG: usize = 100_000;
const HEIGHT: f32 = 220.0;

pub struct Charts {
    sales: Data,
    /// A monthly target over the sales.
    target: Data,
    series: Data,
    long: Data,
    /// The live series' next time and its last value.
    clock: (f64, f64),
    live_x: Vec<f64>,
    live_y: Vec<f64>,
    live: Data,
    noise: u64,
    tick: Option<Task<()>>,
    activation: Option<Subscription>,
    scroll: ScrollHandle,
    bar: TransientState,
}

impl Charts {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"];
        let regions = ["North", "South", "West"];
        let mut noise = 7;
        let (mut month, mut region, mut sales) = (Vec::new(), Vec::new(), Vec::new());
        for (m, name) in months.iter().enumerate() {
            for (r, area) in regions.iter().enumerate() {
                month.push(*name);
                region.push(*area);
                sales.push(20.0 + 6.0 * m as f64 + 8.0 * r as f64 + 10.0 * random(&mut noise));
            }
        }
        let sales = Data::new()
            .text("month", month)
            .text("region", region)
            .number("sales", sales);

        let (mut day, mut name, mut value) = (Vec::new(), Vec::new(), Vec::new());
        let start = chart::time::parse("2026-01-01").unwrap_or_default();
        for (s, series) in ["Requests", "Errors", "Retries"].iter().enumerate() {
            let mut level = 40.0 - 15.0 * s as f64;
            for d in 0..90 {
                level = (level + 6.0 * (random(&mut noise) - 0.5)).max(0.0);
                day.push(start + d as f64 * chart::time::DAY);
                name.push(*series);
                value.push(level);
            }
        }
        let series = Data::new()
            .number("day", day)
            .text("series", name)
            .number("value", value);

        let long = Data::new()
            .number("t", (0..LONG).map(|i| i as f64).collect::<Vec<_>>())
            .number(
                "v",
                (0..LONG)
                    .map(|i| {
                        let t = i as f64 / LONG as f64;
                        (t * 40.0).sin() * 20.0 + (t * 900.0).sin() * 4.0 + random(&mut noise) * 3.0
                    })
                    .collect::<Vec<_>>(),
            );

        let target = Data::new()
            .text("month", months)
            .number("sales", [90.0, 100.0, 110.0, 115.0, 125.0, 140.0]);

        Self {
            sales,
            target,
            series,
            long,
            clock: (0.0, 50.0),
            live_x: Vec::with_capacity(LIVE),
            live_y: Vec::with_capacity(LIVE),
            live: Data::new(),
            noise,
            tick: None,
            activation: None,
            scroll: ScrollHandle::new(),
            bar: TransientState::new(Painter::of(cx)),
        }
    }

    /// One more point on the live series, dropping the oldest past [`LIVE`].
    fn advance(&mut self) {
        let (x, y) = self.clock;
        let y = (y + 8.0 * (random(&mut self.noise) - 0.5)).clamp(0.0, 100.0);
        self.clock = (x + TICK.as_millis() as f64, y);
        if self.live_x.len() == LIVE {
            self.live_x.remove(0);
            self.live_y.remove(0);
        }
        self.live_x.push(x);
        self.live_y.push(y);
        self.live = Data::new()
            .number("t", self.live_x.clone())
            .number("v", self.live_y.clone());
    }

    fn schedule_tick(&mut self, cx: &mut Context<Self>) {
        if self.tick.is_some() {
            return;
        }
        self.tick = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TICK).await;
            let _ = this.update(cx, |this, cx| {
                this.tick = None;
                this.advance();
                cx.notify();
            });
        }));
    }
}

/// A uniform value in `0..1` from a xorshift state.
fn random(state: &mut u64) -> f64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state >> 11) as f64 / (1u64 << 53) as f64
}

fn card(title: &'static str, chart: Chart, theme: &Theme) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .text_style(TextStyle::Caption)
                .text_color(theme.text_faint)
                .child(SharedString::from(title)),
        )
        .child(chart::view(title, chart).h(px(HEIGHT)))
}

impl Render for Charts {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.activation.is_none() {
            self.activation = Some(cx.observe_window_activation(window, |_, _, cx| cx.notify()));
        }
        if window.is_window_active() {
            self.schedule_tick(cx);
        } else {
            self.tick = None;
        }

        let theme = Theme::of(cx).clone();
        let by_day = |chart: Chart| {
            chart
                .x(Channel::temporal("day"))
                .y(Channel::quantitative("value"))
                .color(Channel::nominal("series"))
        };
        let charts = [
            card(
                "Bars, stacked by colour",
                Chart::bar(self.sales.clone())
                    .x(Channel::ordinal("month"))
                    .y(Channel::quantitative("sales"))
                    .color(Channel::nominal("region")),
                &theme,
            ),
            card(
                "Horizontal bars",
                Chart::bar(self.sales.clone())
                    .y(Channel::nominal("region"))
                    .x(Channel::quantitative("sales")),
                &theme,
            ),
            card(
                "Lines over time",
                by_day(Chart::line(self.series.clone())),
                &theme,
            ),
            card("Areas", by_day(Chart::area(self.series.clone())), &theme),
            card("Points", by_day(Chart::point(self.series.clone())), &theme),
            card(
                "Layers: bars under a target line",
                Chart::bar(self.sales.clone())
                    .x(Channel::ordinal("month"))
                    .y(Channel::quantitative("sales"))
                    .color(Channel::nominal("region"))
                    .layer(
                        Chart::line(self.target.clone())
                            .x(Channel::ordinal("month"))
                            .y(Channel::quantitative("sales")),
                    ),
                &theme,
            ),
            card(
                "Arc",
                Chart::arc(self.sales.clone())
                    .theta(Channel::quantitative("sales"))
                    .color(Channel::nominal("region")),
                &theme,
            ),
            card(
                "100,000 points, M4-decimated",
                Chart::line(self.long.clone())
                    .x(Channel::quantitative("t"))
                    .y(Channel::quantitative("v")),
                &theme,
            ),
            card(
                "Live, a point every 100 ms",
                Chart::area(self.live.clone())
                    .x(Channel::temporal("t"))
                    .y(Channel::quantitative("v")),
                &theme,
            ),
        ];

        div()
            .relative()
            .size_full()
            .child(
                scroll::pane("charts-page", Axes::Vertical)
                    .size_full()
                    .track_scroll(&self.scroll)
                    .child(
                        stack()
                            .child(hint(
                                &theme,
                                "chart::view over columnar Data, one element per chart. Hover \
                                 for values; the appearance switch above repaints them in the \
                                 other palette.",
                            ))
                            .child(div().grid().grid_cols(2).gap(px(24.0)).children(charts)),
                    ),
            )
            .child(scroll::transient(
                "charts-bar",
                &self.scroll,
                &self.bar,
                cx.reduce_motion(),
            ))
    }
}
