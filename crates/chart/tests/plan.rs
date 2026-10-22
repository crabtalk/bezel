use chart::{
    Channel, Chart, Data, Encode, Encoding, Mark, Position, Properties, Scale, Shape, Sort,
    decimate::m4,
    model::{
        Align, Axis, Baseline, Dir, Extent, Interpolate, Invalid, Kind, LineBreak, Nice, Orient,
        Overlay, Palette, Range, ScaleType, Scheme as ColorScheme, Side, Stack, TickCount,
    },
    plan::{Face, Fill, Ink, Marks, Metrics, Rect, Symbol, plan},
    scale::Ticks,
    time,
};
use d3::{
    color::Rgb,
    interpolate,
    scale::{Continuous, Transform},
    scheme::Scheme,
    shape::curve::Curve,
    time::{Unit, date::LocalZone},
};
use gpui::{Hsla, Rgba};

const SIZE: [f32; 2] = [400.0, 300.0];
const LINE: f32 = 14.0;
const METRICS: Metrics = Metrics {
    size: 12.0,
    line: LINE,
};

fn measure(text: &str, _: &Face) -> f32 {
    text.chars().count() as f32 * 7.0
}

fn laid_out(chart: &Chart) -> chart::Plan {
    plan(chart, SIZE, METRICS, &mut measure).expect("plans")
}

fn sales() -> Data {
    Data::new()
        .text("month", ["Jan", "Jan", "Feb", "Feb", "Mar"])
        .text("region", ["east", "west", "east", "west", "east"])
        .number("sales", [3.0, 2.0, 5.0, -1.0, 4.0])
}

/// About `count` ticks on a linear axis over `lo..=hi` made nice as Vega
/// makes it, and its domain.
fn linear(lo: f64, hi: f64, count: usize) -> (Ticks, Vec<f64>) {
    let mut scale = Continuous::new(Transform::Linear, vec![lo, hi], vec![0.0, 1.0]);
    scale.nice(10.0);
    (Ticks::of(&scale, count as f64), scale.domain)
}

fn temporal(lo: f64, hi: f64, count: usize) -> Ticks {
    let scale = Continuous::new(
        Transform::Time(LocalZone::utc()),
        vec![lo, hi],
        vec![0.0, 1.0],
    );
    Ticks::of(&scale, count as f64)
}

#[test]
fn linear_ticks_are_round_and_cover_the_data() {
    let (ticks, domain) = linear(0.0, 97.0, 5);
    assert_eq!(ticks.values, [0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
    assert_eq!(domain, [0.0, 100.0]);
    assert_eq!(linear(0.0, 0.3, 3).0.labels, ["0.0", "0.1", "0.2", "0.3"]);
    // Nice to ten ticks, then two of them.
    let (ticks, domain) = linear(0.0, 25_000.0, 2);
    assert_eq!(domain, [0.0, 26_000.0]);
    assert_eq!(ticks.labels, ["0", "10,000", "20,000"]);
    assert_eq!(linear(0.0, 4_000.0, 2).0.labels, ["0", "2,000", "4,000"]);
}

#[test]
fn values_read_out_as_vega_lite_writes_them() {
    let quantity = |v: f64| Kind::Quantitative.describe(v);
    assert_eq!(quantity(1234567.0), "1234567");
    assert_eq!(quantity(0.1 + 0.2), "0.3");
    assert_eq!(quantity(-12.345), "\u{2212}12.345");
    assert_eq!(quantity(0.001234), "0.001234");
    assert_eq!(Kind::Nominal.describe(-1.5), "-1.5");
}

#[test]
fn time_reads_and_describes_utc() {
    let at = time::parse("2024-03-05T14:30:00Z").unwrap();
    assert_eq!(Kind::Temporal.describe(at), "Mar 05, 2024");
    assert_eq!(
        time::parse("2024-03-05T14:30+02:00"),
        time::parse("2024-03-05T12:30Z")
    );
}

#[test]
fn time_ticks_land_on_calendar_boundaries() {
    let lo = time::parse("2024-01-10").unwrap();
    let hi = time::parse("2024-12-20").unwrap();
    let ticks = temporal(lo, hi, 4);
    assert_eq!(ticks.labels, ["April", "July", "October"]);

    let (lo, hi) = (
        time::parse("2020-06-01").unwrap(),
        time::parse("2024-06-01").unwrap(),
    );
    assert_eq!(temporal(lo, hi, 4).labels, ["2021", "2022", "2023", "2024"]);

    let lo = time::parse("2024-03-05T09:10").unwrap();
    let ticks = temporal(lo, lo + 3.0 * time::HOUR, 3);
    assert_eq!(ticks.labels, ["10 AM", "11 AM", "12 PM"]);
}

#[test]
fn m4_keeps_each_columns_extremes_in_order() {
    let points: Vec<[f32; 2]> = (0..1000)
        .map(|i| [i as f32 / 100.0, ((i * 37) % 101) as f32])
        .collect();
    let kept = m4(&points);
    assert!(kept.len() <= 4 * 10);
    for column in 0..10 {
        let all = points.iter().filter(|p| p[0].floor() == column as f32);
        let low = all.clone().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let high = all.map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
        let kept: Vec<f32> = kept
            .iter()
            .filter(|p| p[0].floor() == column as f32)
            .map(|p| p[1])
            .collect();
        assert!(kept.contains(&low) && kept.contains(&high));
    }
    assert!(kept.is_sorted_by(|a, b| a[0] <= b[0]));
}

#[test]
fn bars_stack_up_and_down_from_zero() {
    let chart = Chart::bar(sales())
        .x(Channel::nominal("month"))
        .y(Channel::quantitative("sales"))
        .color(Channel::nominal("region"));
    let plan = laid_out(&chart);
    assert_eq!(plan.legends[0].swatches.len(), 2);
    let bars = |s: usize| match &plan.series[s].marks {
        Marks::Bars(bars) => bars.iter().map(|bar| bar.rect).collect::<Vec<_>>(),
        other => panic!("bars, not {other:?}"),
    };
    let (east, west) = (bars(0), bars(1));
    assert_eq!((east.len(), west.len()), (3, 2));
    // Jan: a vertical stack runs down its colours, so east sits on west.
    assert!((east[0].bottom() - west[0].y).abs() < 0.01);
    // Feb: west is negative, below zero, where east starts.
    assert!((west[1].y - east[1].bottom()).abs() < 0.01);
    assert!(
        east.iter()
            .chain(&west)
            .all(|bar| plan.plot.contains([bar.x, bar.y]))
    );
}

#[test]
fn hovering_a_band_reads_out_each_series() {
    let chart = Chart::bar(sales())
        .x(Channel::nominal("month"))
        .y(Channel::quantitative("sales"))
        .color(Channel::nominal("region"));
    let plan = laid_out(&chart);
    let Marks::Bars(east) = &plan.series[0].marks else {
        panic!()
    };
    let east: Vec<_> = east.iter().map(|bar| bar.rect).collect();
    let feb = east[1];
    let tip = plan.hit([feb.x + 1.0, plan.plot.y + 1.0]).unwrap();
    assert_eq!(tip.title, "Feb");
    assert_eq!(tip.rows, [(0, "5".into()), (1, "\u{2212}1".into())]);
    assert!(plan.hit([0.0, 0.0]).is_none());
}

#[test]
fn horizontal_bars_put_categories_on_the_left() {
    let chart = Chart::bar(sales())
        .y(Channel::ordinal("month"))
        .x(Channel::quantitative("sales"));
    let plan = laid_out(&chart);
    let left = plan.axes.iter().find(|a| a.side == Side::Left).unwrap();
    let labels: Vec<_> = left.labels.iter().map(|l| l.text.as_ref()).collect();
    assert_eq!(labels, ["Jan", "Feb", "Mar"]);
    assert!(left.grid.is_empty());
}

#[test]
fn a_long_line_is_bounded_by_the_plot_width() {
    let n = 100_000;
    let data = Data::new()
        .number("t", (0..n).map(|i| i as f64).collect::<Vec<_>>())
        .number(
            "v",
            (0..n)
                .map(|i| ((i as f64) * 0.01).sin())
                .collect::<Vec<_>>(),
        );
    let plan = laid_out(
        &Chart::line(data)
            .x(Channel::quantitative("t"))
            .y(Channel::quantitative("v")),
    );
    let Marks::Lines(runs) = &plan.series[0].marks else {
        panic!()
    };
    let points: usize = runs.iter().map(Vec::len).sum();
    assert!(points <= 4 * (plan.plot.w.ceil() as usize + 1), "{points}");

    let tip = plan
        .hit([plan.plot.x + plan.plot.w / 2.0, plan.plot.y + 5.0])
        .unwrap();
    assert_eq!(tip.points.len(), 1);
}

#[test]
fn a_missing_value_breaks_the_line() {
    let data = Data::new()
        .number("x", [1.0, 2.0, 3.0, 4.0, 5.0])
        .number("y", [1.0, 2.0, f64::NAN, 4.0, 5.0]);
    let plan = laid_out(
        &Chart::line(data)
            .x(Channel::quantitative("x"))
            .y(Channel::quantitative("y")),
    );
    let Marks::Lines(runs) = &plan.series[0].marks else {
        panic!()
    };
    assert_eq!(runs.iter().map(Vec::len).collect::<Vec<_>>(), [2, 2]);
}

#[test]
fn rows_out_of_order_draw_in_x_order() {
    let data = Data::new()
        .number("x", [3.0, 1.0, 2.0])
        .number("y", [3.0, 1.0, 2.0]);
    let plan = laid_out(
        &Chart::line(data)
            .x(Channel::quantitative("x"))
            .y(Channel::quantitative("y")),
    );
    let Marks::Lines(runs) = &plan.series[0].marks else {
        panic!()
    };
    assert!(runs[0].is_sorted_by(|a, b| a[0] <= b[0]));
}

#[test]
fn an_area_closes_along_zero() {
    let data = Data::new()
        .number("x", [0.0, 1.0, 2.0])
        .number("y", [2.0, 3.0, 1.0]);
    let plan = laid_out(
        &Chart::area(data)
            .x(Channel::quantitative("x"))
            .y(Channel::quantitative("y")),
    );
    let Marks::Areas(areas) = &plan.series[0].marks else {
        panic!()
    };
    // A stack of one: its base runs under each point, along zero.
    let outline = areas[0].outline();
    assert_eq!(outline.len(), 6);
    assert_eq!(outline[3][1], plan.plot.bottom());
    assert_eq!(outline[5][1], plan.plot.bottom());
}

#[test]
fn slices_share_the_circle_by_value() {
    let data = Data::new()
        .text("kind", ["a", "b", "a", "c"])
        .number("n", [1.0, 2.0, 1.0, 4.0]);
    let plan = laid_out(
        &Chart::arc(data)
            .theta(Channel::quantitative("n"))
            .color(Channel::nominal("kind")),
    );
    let spans: Vec<f32> = plan
        .series
        .iter()
        .map(|s| match &s.marks {
            Marks::Slices(slices) => slices.iter().map(|s| s.end - s.start).sum(),
            _ => panic!(),
        })
        .collect();
    let tau = std::f32::consts::TAU;
    assert!((spans[0] - tau * 0.25).abs() < 1e-4 && (spans[2] - tau * 0.5).abs() < 1e-4);

    // Just right of twelve o'clock is the first slice.
    let Marks::Slices(first) = &plan.series[0].marks else {
        panic!()
    };
    let [cx, cy] = first[0].center;
    let tip = plan.hit([cx + 2.0, cy - 10.0]).unwrap();
    assert_eq!(tip.title, "a");
    assert_eq!(tip.rows[0].1, "1 (12.5%)");
}

#[test]
fn time_on_x_labels_by_the_calendar() {
    let days: Vec<f64> = (0..90)
        .map(|d| time::parse("2024-01-01").unwrap() + d as f64 * time::DAY)
        .collect();
    let data = Data::new().number("day", days).number("v", vec![1.0; 90]);
    let plan = laid_out(
        &Chart::line(data)
            .x(Channel::temporal("day"))
            .y(Channel::quantitative("v")),
    );
    let bottom = plan.axes.iter().find(|a| a.side == Side::Bottom).unwrap();
    // A tick each Sunday, about one to 40 pixels.
    assert_eq!(bottom.labels[0].text, "Jan 07");
    let tip = plan.hit([plan.plot.x + 1.0, plan.plot.y + 1.0]).unwrap();
    assert_eq!(tip.title, "Jan 01, 2024");
}

#[test]
fn an_encoding_the_mark_cannot_draw_is_none() {
    let chart = |c: Chart| plan(&c, SIZE, METRICS, &mut measure).is_none();
    assert!(chart(
        Chart::bar(sales())
            .x(Channel::nominal("month"))
            .y(Channel::quantitative("missing"))
    ));
    assert!(chart(
        Chart::line(sales())
            .x(Channel::quantitative("month"))
            .y(Channel::quantitative("sales"))
    ));
    assert!(chart(
        Chart::area(sales()).x(Channel::quantitative("sales"))
    ));
}

#[test]
fn no_size_changes_whether_a_chart_draws() {
    let tiny = Chart::bar(sales())
        .x(Channel::nominal("month"))
        .y(Channel::quantitative("sales"));
    let plan = plan(&tiny, [20.0, 20.0], METRICS, &mut measure).expect("plans");
    assert!(plan.plot.w > 0.0 && plan.plot.h > 0.0);
    assert!(tiny.drawable());
    assert!(
        !Chart::line(sales())
            .x(Channel::quantitative("month"))
            .y(Channel::quantitative("sales"))
            .drawable()
    );
}

fn temperatures() -> Data {
    Data::new()
        .number("x", [0.0, 1.0, 2.0])
        .number("t", [21.0, 24.0, 22.5])
}

fn left_labels(plan: &chart::Plan) -> Vec<String> {
    let left = plan.axes.iter().find(|a| a.side == Side::Left).unwrap();
    left.labels.iter().map(|l| l.text.to_string()).collect()
}

#[test]
fn zero_off_lets_the_scale_start_at_the_data() {
    let with_zero = laid_out(
        &Chart::line(temperatures())
            .x(Channel::quantitative("x"))
            .y(Channel::quantitative("t")),
    );
    assert_eq!(left_labels(&with_zero).first().unwrap(), "0");
    let without = laid_out(
        &Chart::line(temperatures())
            .x(Channel::quantitative("x"))
            .y(Channel::quantitative("t").zero(false)),
    );
    let labels = left_labels(&without);
    assert_eq!(
        (
            labels.first().unwrap().as_str(),
            labels.last().unwrap().as_str()
        ),
        ("21.0", "24.0")
    );
}

#[test]
fn a_fixed_domain_is_the_scale_exactly() {
    let plan = laid_out(
        &Chart::line(temperatures())
            .x(Channel::quantitative("x"))
            .y(Channel::quantitative("t").domain(0.0, 30.0)),
    );
    let Marks::Lines(runs) = &plan.series[0].marks else {
        panic!()
    };
    let top = plan.plot.y;
    let expected = top + plan.plot.h * (1.0 - 24.0 / 30.0);
    assert!((runs[0][1][1] - expected).abs() < 0.01);
}

#[test]
fn sort_orders_a_discrete_domain() {
    let order = |sort: Sort| {
        let plan = laid_out(
            &Chart::bar(sales())
                .y(Channel::nominal("month").sort(sort))
                .x(Channel::quantitative("sales")),
        );
        left_labels(&plan)
    };
    assert_eq!(order(Sort::Data), ["Jan", "Feb", "Mar"]);
    assert_eq!(order(Sort::Ascending), ["Feb", "Jan", "Mar"]);
    assert_eq!(order(Sort::Descending), ["Mar", "Jan", "Feb"]);
    assert_eq!(
        order(Sort::Explicit(vec!["Mar".into()])),
        ["Mar", "Jan", "Feb"]
    );
}

#[test]
fn titles_label_their_axes_and_the_legend() {
    let plan = laid_out(
        &Chart::bar(sales())
            .x(Channel::nominal("month").title("Month"))
            .y(Channel::quantitative("sales").title("Sales"))
            .color(Channel::nominal("region").title("Region")),
    );
    let title = |side| {
        let axis = plan.axes.iter().find(|a| a.side == side).unwrap();
        axis.title.clone().unwrap()
    };
    let (x, y) = (title(Side::Bottom), title(Side::Left));
    assert_eq!((x.text.as_ref(), y.text.as_ref()), ("Month", "Sales"));
    assert!(x.at[1] > plan.plot.bottom() && y.at[1] + LINE <= plan.plot.y + 0.01);
    // The legend stands right of the plot, its title over its entries.
    let legend = &plan.legends[0];
    let title = legend.title.as_ref().unwrap();
    assert_eq!(title.text, "Region");
    assert!(title.at[0] > plan.plot.right());
    assert!(legend.swatches[0].swatch.y > title.at[1] + title.height());
}

#[test]
fn layers_share_their_scales_and_colours() {
    let actual = Data::new()
        .text("month", ["Jan", "Feb", "Mar"])
        .number("v", [3.0, 5.0, 4.0]);
    let target = Data::new()
        .text("month", ["Feb", "Mar", "Apr"])
        .number("v", [10.0, 6.0, 7.0]);
    let chart = Chart::bar(actual)
        .x(Channel::nominal("month"))
        .y(Channel::quantitative("v"))
        .layer(
            Chart::line(target)
                .x(Channel::nominal("month"))
                .y(Channel::quantitative("v")),
        );
    let plan = laid_out(&chart);
    let bottom = plan.axes.iter().find(|a| a.side == Side::Bottom).unwrap();
    let months: Vec<_> = bottom.labels.iter().map(|l| l.text.as_ref()).collect();
    assert_eq!(months, ["Jan", "Feb", "Mar", "Apr"]);
    // The line's 10 sets the top, which the bars are measured against.
    assert_eq!(left_labels(&plan).last().unwrap(), "10");
    let colors: Vec<usize> = plan.series.iter().map(|s| s.color).collect();
    assert_eq!(colors, [0, 1]);
    assert!(plan.legends.is_empty());

    // Over Feb: the bar's band and the line's point read out together.
    let Marks::Bars(bars) = &plan.series[0].marks else {
        panic!()
    };
    let bars: Vec<_> = bars.iter().map(|bar| bar.rect).collect();
    let tip = plan.hit([bars[1].x + 1.0, plan.plot.y + 1.0]).unwrap();
    assert_eq!(tip.title, "Feb");
    assert_eq!(tip.rows, [(0, "5".into()), (1, "10".into())]);
    assert!(tip.band.is_some() && tip.points.len() == 1);
}

#[test]
fn a_colour_value_is_one_colour_across_layers() {
    let data = Data::new()
        .text("month", ["Jan", "Jan", "Feb", "Feb"])
        .text("region", ["east", "west", "east", "west"])
        .number("v", [1.0, 2.0, 3.0, 4.0]);
    let west = Data::new()
        .text("month", ["Jan", "Feb"])
        .text("region", ["west", "west"])
        .number("v", [5.0, 6.0]);
    let layer = |chart: Chart| {
        chart
            .x(Channel::nominal("month"))
            .y(Channel::quantitative("v"))
            .color(Channel::nominal("region"))
    };
    let plan = laid_out(&layer(Chart::bar(data)).layer(layer(Chart::line(west))));
    let names: Vec<_> = plan.colors.iter().map(|c| c.name.clone()).collect();
    assert_eq!(names, [Some("east".into()), Some("west".into())]);
    let line = plan
        .series
        .iter()
        .find(|s| s.mark == chart::Mark::Line)
        .unwrap();
    assert_eq!(line.color, 1);
}

#[test]
fn layers_that_disagree_are_none_and_a_pie_over_axes_stands_mid_plot() {
    let line = Chart::line(temperatures())
        .x(Channel::quantitative("x"))
        .y(Channel::quantitative("t"));
    let bars = Chart::bar(sales())
        .x(Channel::nominal("month"))
        .y(Channel::quantitative("sales"));
    assert!(
        plan(
            &bars.clone().layer(line.clone()),
            SIZE,
            METRICS,
            &mut measure
        )
        .is_none()
    );
    let pie = Chart::arc(sales()).theta(Channel::quantitative("sales"));
    let plan = laid_out(&line.layer(pie));
    let Marks::Slices(slices) = &plan.series[1].marks else {
        panic!()
    };
    let plot = plan.plot;
    assert_eq!(
        slices[0].center,
        [plot.x + plot.w / 2.0, plot.y + plot.h / 2.0]
    );
}

#[test]
fn an_offset_puts_bars_side_by_side_within_their_band() {
    let chart = Chart::bar(sales())
        .x(Channel::nominal("month"))
        .y(Channel::quantitative("sales"))
        .x_offset(Channel::nominal("region"))
        .color(Channel::nominal("region"));
    let plan = laid_out(&chart);
    let bars = |s: usize| match &plan.series[s].marks {
        Marks::Bars(bars) => bars.iter().map(|bar| bar.rect).collect::<Vec<_>>(),
        other => panic!("bars, not {other:?}"),
    };
    let (east, west) = (bars(0), bars(1));
    // Jan: west beside east, both from zero.
    assert!((west[0].x - east[0].right()).abs() < 0.01);
    assert!((west[0].bottom() - east[0].bottom()).abs() < 0.01);
}

#[test]
fn x2_ranges_a_bar_instead_of_stacking_it() {
    let data = Data::new()
        .text("task", ["a", "b"])
        .number("start", [1.0, 3.0])
        .number("end", [4.0, 8.0]);
    let plan = laid_out(
        &Chart::bar(data)
            .y(Channel::ordinal("task"))
            .x(Channel::quantitative("start"))
            .x2(Channel::quantitative("end")),
    );
    let Marks::Bars(bars) = &plan.series[0].marks else {
        panic!()
    };
    let bars: Vec<_> = bars.iter().map(|bar| bar.rect).collect();
    // 3 wide from 1, and 5 wide from 3.
    assert!((bars[1].w / bars[0].w - 5.0 / 3.0).abs() < 1e-3);
    assert!(((bars[1].x - bars[0].x) / bars[0].w - 2.0 / 3.0).abs() < 1e-3);
    let tip = plan.hit([bars[1].x + 1.0, bars[1].y + 1.0]).unwrap();
    assert_eq!(tip.rows[0].1, "3 – 8");
}

#[test]
fn y2_bounds_an_area_from_below() {
    let data = Data::new()
        .number("x", [0.0, 1.0, 2.0])
        .number("lo", [1.0, 2.0, 1.0])
        .number("hi", [3.0, 4.0, 5.0]);
    let plan = laid_out(
        &Chart::area(data)
            .x(Channel::quantitative("x"))
            .y(Channel::quantitative("hi"))
            .y2(Channel::quantitative("lo")),
    );
    let Marks::Areas(areas) = &plan.series[0].marks else {
        panic!()
    };
    assert_eq!((areas[0].top.len(), areas[0].base.len()), (3, 3));
    let outline = areas[0].outline();
    assert!(outline[3][1] > outline[2][1]);
}

#[test]
fn detail_and_dash_split_lines_without_a_colour() {
    let data = Data::new()
        .number("x", [1.0, 2.0, 1.0, 2.0])
        .number("y", [1.0, 2.0, 3.0, 4.0])
        .text("kind", ["a", "a", "b", "b"]);
    let line = |chart: Chart| {
        laid_out(
            &chart
                .x(Channel::quantitative("x"))
                .y(Channel::quantitative("y")),
        )
    };
    let detail = line(Chart::line(data.clone()).detail(Channel::nominal("kind")));
    assert_eq!(detail.series.len(), 2);
    assert!(detail.series.iter().all(|s| s.color == 0));
    assert!(detail.legends.is_empty());

    let dashed = line(Chart::line(data).stroke_dash(Channel::nominal("kind")));
    let dashes: Vec<_> = dashed.series.iter().map(|s| s.style.dash.clone()).collect();
    assert!(dashes[0].is_none());
    assert_eq!(dashes[1].as_deref(), Some(&[4.0, 2.0][..]));
    let glyph = &dashed.legends[0].swatches[1].glyph;
    assert_eq!(glyph.shape, Shape::Stroke);
    assert_eq!(glyph.dash.as_deref(), Some(&[4.0, 2.0][..]));
}

#[test]
fn order_joins_a_line_in_its_own_order() {
    let data = Data::new()
        .number("x", [1.0, 3.0, 2.0])
        .number("y", [1.0, 2.0, 3.0])
        .number("step", [0.0, 1.0, 2.0]);
    let plan = laid_out(
        &Chart::line(data)
            .x(Channel::quantitative("x"))
            .y(Channel::quantitative("y"))
            .order(Channel::quantitative("step")),
    );
    let Marks::Lines(runs) = &plan.series[0].marks else {
        panic!()
    };
    assert!(runs[0][1][0] > runs[0][2][0]);
}

#[test]
fn points_take_their_size_shape_and_opacity() {
    let data = Data::new()
        .number("x", [1.0, 2.0, 3.0])
        .number("y", [1.0, 2.0, 3.0])
        .number("weight", [0.0, 5.0, 10.0])
        .text("kind", ["a", "b", "a"]);
    let plan = laid_out(
        &Chart::point(data)
            .x(Channel::quantitative("x"))
            .y(Channel::quantitative("y"))
            .size(Channel::quantitative("weight"))
            .shape(Channel::nominal("kind"))
            .opacity(Encode::Value(0.5)),
    );
    let Marks::Points(points) = &plan.series[0].marks else {
        panic!()
    };
    let sizes: Vec<f32> = points.iter().map(|p| p.size).collect();
    assert_eq!(sizes, [4.0, 182.5, 361.0]);
    let shapes: Vec<Shape> = points.iter().map(|p| p.shape.clone()).collect();
    assert_eq!(shapes, [Shape::Circle, Shape::Square, Shape::Circle]);
    assert!(points.iter().all(|p| p.style.opacity == 0.5));
    // A size legend, then a shape legend, as Vega-Lite orders them.
    assert_eq!(plan.legends.len(), 2);
    assert_eq!(plan.legends[1].swatches[1].glyph.shape, Shape::Square);
}

#[test]
fn radius2_hollows_a_pie_into_a_donut() {
    let data = Data::new().text("kind", ["a", "b"]).number("n", [1.0, 3.0]);
    let plan = laid_out(
        &Chart::arc(data)
            .theta(Channel::quantitative("n"))
            .color(Channel::nominal("kind"))
            .radius2(Encode::Value(40.0)),
    );
    let Marks::Slices(slices) = &plan.series[1].marks else {
        panic!()
    };
    let slice = slices[0];
    assert_eq!(slice.inner, 40.0);
    let [cx, cy] = slice.center;
    assert!(plan.hit([cx + 1.0, cy + 20.0]).is_none());
    assert!(plan.hit([cx + 1.0, cy + 60.0]).is_some());
}

#[test]
fn a_mark_with_an_href_reports_it_where_it_is_drawn() {
    let chart = Chart::bar(sales())
        .x(Channel::nominal("month"))
        .y(Channel::quantitative("sales"))
        .href(Encode::Value("https://example.com".into()));
    let plan = laid_out(&chart);
    let Marks::Bars(bars) = &plan.series[0].marks else {
        panic!()
    };
    let bars: Vec<_> = bars.iter().map(|bar| bar.rect).collect();
    let bar = bars[0];
    let inside = [bar.x + bar.w / 2.0, bar.y + bar.h / 2.0];
    assert_eq!(
        plan.href(inside).map(|h| h.as_ref()),
        Some("https://example.com")
    );
    assert!(plan.href([0.0, 0.0]).is_none());
}

fn xy() -> Data {
    Data::new()
        .text("month", ["Jan", "Feb", "Mar"])
        .number("x", [1.0, 2.0, 3.0])
        .number("y", [2.0, 5.0, 3.0])
        .number("y2", [1.0, 1.0, 2.0])
        .text("label", ["a", "b", ""])
        .text("url", ["a.png", "b.png", ""])
}

fn mark(mark: Mark) -> Chart {
    Chart::new(mark, xy())
        .x(Channel::quantitative("x"))
        .y(Channel::quantitative("y"))
}

#[test]
fn a_rule_with_one_position_spans_the_plot() {
    let chart = mark(Mark::Line).layer(
        Chart::new(Mark::Rule, xy())
            .y(Channel::datum(4.0, chart::Kind::Quantitative))
            .stroke_dash(Encode::Value(vec![2.0, 2.0]))
            .size(Encode::Value(2.0)),
    );
    let plan = laid_out(&chart);
    let rule = &plan.series[1];
    let Marks::Rules(rules) = &rule.marks else {
        panic!("rules, not {:?}", rule.marks)
    };
    assert_eq!(rules[0][0][0], plan.plot.x);
    assert_eq!(rules[0][1][0], plan.plot.right());
    assert_eq!(rules[0][0][1], rules[0][1][1]);
    assert_eq!(rule.style.stroke_width, 2.0);
    // A rule with no colour of its own is the theme's text.
    assert_eq!(plan.colors[rule.color].ink, Ink::Text);

    // With y2, from y to y2 at each x.
    let ranged = laid_out(&mark(Mark::Rule).y2(Channel::quantitative("y2")));
    let Marks::Rules(rules) = &ranged.series[0].marks else {
        panic!()
    };
    assert_eq!(rules.len(), 3);
    assert_eq!(rules[0][0][0], rules[0][1][0]);
    assert!(rules[0][0][1] < rules[0][1][1]);
}

#[test]
fn ticks_cross_their_band_thin_along_the_values() {
    let plan = laid_out(
        &Chart::new(Mark::Tick, xy())
            .x(Channel::nominal("month"))
            .y(Channel::quantitative("y")),
    );
    let Marks::Bars(ticks) = &plan.series[0].marks else {
        panic!()
    };
    let step = plan.plot.w / 3.0;
    assert!((ticks[0].rect.w - step * 0.75).abs() < 0.01);
    assert_eq!(ticks[0].rect.h, 1.0);
    assert_eq!(plan.series[0].style.opacity, 0.7);

    let thick = laid_out(
        &Chart::new(Mark::Tick, xy())
            .x(Channel::nominal("month"))
            .y(Channel::quantitative("y"))
            .properties(Properties {
                thickness: Some(3.0),
                width: Some(Extent::Band(0.5)),
                ..Properties::default()
            }),
    );
    let Marks::Bars(ticks) = &thick.series[0].marks else {
        panic!()
    };
    assert!((ticks[0].rect.w - step * 0.75 * 0.5).abs() < 0.01);
    assert_eq!(ticks[0].rect.h, 3.0);
}

#[test]
fn rects_fill_their_ranges() {
    let plan = laid_out(
        &mark(Mark::Rect)
            .x2(Channel::quantitative("y2"))
            .y2(Channel::quantitative("y2")),
    );
    let Marks::Bars(rects) = &plan.series[0].marks else {
        panic!()
    };
    assert_eq!(rects.len(), 3);
    assert_eq!(rects[0].corners, [0.0; 4]);
    // A rect without its second ends draws nothing.
    assert!(plan_of(&mark(Mark::Rect)).is_none());
}

fn plan_of(chart: &Chart) -> Option<chart::Plan> {
    plan(chart, SIZE, METRICS, &mut measure)
}

#[test]
fn texts_stand_at_their_rows_offset_and_aligned() {
    let plan = laid_out(
        &mark(Mark::Text)
            .text(Channel::nominal("label"))
            .properties(Properties {
                dx: 3.0,
                align: Some(Align::Left),
                font_size: Some(14.0),
                ..Properties::default()
            }),
    );
    let points = laid_out(&mark(Mark::Point));
    let (Marks::Texts(texts, font), Marks::Points(symbols)) =
        (&plan.series[0].marks, &points.series[0].marks)
    else {
        panic!()
    };
    // The empty text draws nothing.
    assert_eq!(texts.len(), 2);
    assert_eq!(texts[0].at[0], symbols[0].at[0] + 3.0);
    assert_eq!(texts[0].size, 14.0);
    assert_eq!((font.align, font.baseline), (Align::Left, Baseline::Middle));
    assert!(plan_of(&mark(Mark::Text).angle(Encode::Value(30.0))).is_none());
}

#[test]
fn a_trail_widens_with_its_size() {
    let plan = laid_out(&mark(Mark::Trail).size(Channel::quantitative("y")));
    let Marks::Trails(trails) = &plan.series[0].marks else {
        panic!()
    };
    let widths: Vec<f32> = trails[0].iter().map(|(_, width)| *width).collect();
    assert!(widths[0] < widths[2] && widths[2] < widths[1]);
    assert!(plan.series[0].style.fill.is_some());
}

#[test]
fn images_stand_at_their_rows() {
    let plan = laid_out(
        &mark(Mark::Image)
            .url(Channel::nominal("url"))
            .properties(Properties {
                width: Some(Extent::Pixels(20.0)),
                ..Properties::default()
            }),
    );
    let Marks::Images(images) = &plan.series[0].marks else {
        panic!()
    };
    assert_eq!(images.len(), 2);
    assert_eq!(images[0].url.as_ref(), "a.png");
    assert_eq!((images[0].width, images[0].height), (Some(20.0), None));
    assert!(images[0].aspect);
}

#[test]
fn a_point_is_hollow_and_a_circle_or_square_filled() {
    let point = laid_out(&mark(Mark::Point));
    let Marks::Points(symbols) = &point.series[0].marks else {
        panic!()
    };
    assert!(symbols[0].style.fill.is_none() && symbols[0].style.stroke.is_some());
    assert_eq!(symbols[0].style.opacity, 0.7);
    for (mark, shape) in [(Mark::Circle, Shape::Circle), (Mark::Square, Shape::Square)] {
        let plan = laid_out(&self::mark(mark).shape(Encode::Value(Shape::Cross)));
        let Marks::Points(symbols) = &plan.series[0].marks else {
            panic!()
        };
        assert_eq!(symbols[0].shape, shape);
        assert!(symbols[0].style.fill.is_some() && symbols[0].style.stroke.is_none());
    }
}

#[test]
fn lines_and_areas_take_their_curve_and_overlays() {
    let overlay = Overlay {
        properties: Properties {
            filled: Some(true),
            ..Properties::default()
        },
        encoding: Encoding {
            size: Some(Encode::Value(50.0)),
            ..Encoding::default()
        },
    };
    let plan = laid_out(&mark(Mark::Area).properties(Properties {
        interpolate: Interpolate::Monotone,
        line: Some(Box::new(Overlay::default())),
        point: Some(Box::new(overlay)),
        ..Properties::default()
    }));
    let marks: Vec<Mark> = plan.series.iter().map(|s| s.mark).collect();
    assert_eq!(marks, [Mark::Area, Mark::Line, Mark::Point]);
    assert_eq!(plan.series[0].curve, Curve::MonotoneX);
    assert_eq!(plan.series[1].curve, Curve::Linear);
    let Marks::Points(points) = &plan.series[2].marks else {
        panic!()
    };
    assert_eq!(points.len(), 3);
    assert_eq!(points[0].size, 50.0);
    assert!(points[0].style.fill.is_some());
    assert!(plan.series.iter().all(|s| s.color == plan.series[0].color));
}

#[test]
fn bar_corners_round_as_their_properties_say() {
    let corner = |properties: Properties| {
        let plan = laid_out(
            &Chart::bar(sales())
                .x(Channel::nominal("month"))
                .y(Channel::quantitative("sales"))
                .color(Channel::nominal("region"))
                .properties(properties),
        );
        let corners = |s: usize| match &plan.series[s].marks {
            Marks::Bars(bars) => bars.iter().map(|bar| bar.corners).collect::<Vec<_>>(),
            _ => panic!(),
        };
        (corners(0), corners(1))
    };
    // Bezel's own corners, with no property.
    assert_eq!(corner(Properties::default()).0[0], [2.0; 4]);
    let (east, west) = corner(Properties {
        corner_radius_end: Some(4.0),
        ..Properties::default()
    });
    // Jan stacks east on west: east's top is the stack's end.
    assert_eq!(east[0], [4.0, 4.0, 0.0, 0.0]);
    assert_eq!(west[0], [0.0; 4]);
    // Feb's west is below zero: its bottom is the end.
    assert_eq!(west[1], [0.0, 0.0, 4.0, 4.0]);
    let (east, _) = corner(Properties {
        corner_radius: [Some(1.0), None, None, Some(3.0)],
        ..Properties::default()
    });
    assert_eq!(east[0], [1.0, 0.0, 0.0, 3.0]);
}

#[test]
fn text_labels_a_pie_at_its_slices() {
    let data = Data::new().text("kind", ["a", "b"]).number("n", [1.0, 3.0]);
    let pie = |mark: Mark| {
        Chart::new(mark, data.clone())
            .theta(Channel::quantitative("n"))
            .color(Channel::nominal("kind"))
    };
    let plan = laid_out(
        &pie(Mark::Arc).layer(
            pie(Mark::Text)
                .text(Channel::nominal("kind"))
                .radius(Encode::Value(50.0)),
        ),
    );
    let texts: Vec<&chart::plan::Text> = plan
        .series
        .iter()
        .filter_map(|s| match &s.marks {
            Marks::Texts(texts, _) => Some(texts),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(texts.len(), 2);
    let center = [
        plan.plot.x + plan.plot.w / 2.0,
        plan.plot.y + plan.plot.h / 2.0,
    ];
    let distance = (texts[0].at[0] - center[0]).hypot(texts[0].at[1] - center[1]);
    assert!((distance - 50.0).abs() < 0.01);
    // "a" spans the first quarter, so stands right of centre and up.
    assert!(texts[0].at[0] > center[0] && texts[0].at[1] < center[1]);
    assert!(plan_of(&pie(Mark::Arc).layer(pie(Mark::Bar))).is_none());
}

#[test]
fn offsets_move_a_marks_ends_apart() {
    let ranged = |properties: Properties| {
        let plan = laid_out(
            &mark(Mark::Rule)
                .x2(Channel::quantitative("y2"))
                .y2(Channel::quantitative("y2"))
                .x_offset(Encode::Value(3.0))
                .properties(properties),
        );
        let Marks::Rules(rules) = &plan.series[0].marks else {
            panic!()
        };
        rules[0]
    };
    let plain = ranged(Properties::default());
    let moved = ranged(Properties {
        x2_offset: -2.0,
        y2_offset: 5.0,
        ..Properties::default()
    });
    // `xOffset` moves x alone; `x2Offset` and `y2Offset` the far end.
    assert_eq!(moved[0], plain[0]);
    assert_eq!(moved[1], [plain[1][0] - 2.0, plain[1][1] + 5.0]);

    // Without an x2, a bar's far end moves with the offset: the whole bar.
    let bars = |offset: f32| {
        let plan = laid_out(
            &Chart::bar(sales())
                .x(Channel::nominal("month"))
                .y(Channel::quantitative("sales"))
                .x_offset(Encode::Value(offset)),
        );
        let Marks::Bars(bars) = &plan.series[0].marks else {
            panic!()
        };
        bars[0].rect
    };
    let (still, shifted) = (bars(0.0), bars(4.0));
    assert_eq!((shifted.x, shifted.w), (still.x + 4.0, still.w));
}

#[test]
fn positions_in_pixels_stand_in_the_plot() {
    let chart = mark(Mark::Point)
        .layer(
            Chart::new(Mark::Rect, xy())
                .x(Encode::Value(Position::Pixels(0.0)))
                .x2(Encode::Value(Position::Size))
                .y(Channel::quantitative("y"))
                .y2(Channel::quantitative("y2")),
        )
        .layer(
            Chart::new(Mark::Text, xy())
                .x(Encode::Value(Position::Pixels(10.0)))
                .y(Encode::Value(Position::Size))
                .text(Encode::Value("note".into())),
        )
        .layer(Chart::new(Mark::Rule, xy()).y(Encode::Value(Position::Pixels(20.0))));
    let plan = laid_out(&chart);
    let plot = plan.plot;
    let Marks::Bars(rects) = &plan.series[1].marks else {
        panic!()
    };
    assert_eq!(
        (rects[0].rect.x, rects[0].rect.right()),
        (plot.x, plot.right())
    );
    let Marks::Texts(texts, _) = &plan.series[2].marks else {
        panic!()
    };
    assert_eq!(texts[0].at, [plot.x + 10.0, plot.bottom()]);
    let Marks::Rules(rules) = &plan.series[3].marks else {
        panic!()
    };
    assert_eq!(
        rules[0],
        [[plot.x, plot.y + 20.0], [plot.right(), plot.y + 20.0]]
    );
}

#[test]
fn horizontal_charts_hover_along_y() {
    let data = Data::new()
        .text("month", ["Jan", "Feb", "Mar"])
        .number("sales", [3.0, 5.0, 4.0]);
    let plan = laid_out(
        &Chart::line(data)
            .y(Channel::nominal("month"))
            .x(Channel::quantitative("sales")),
    );
    let Marks::Lines(lines) = &plan.series[0].marks else {
        panic!()
    };
    let feb = lines[0][1];
    let tip = plan.hit([plan.plot.x + 1.0, feb[1] + 2.0]).expect("a tip");
    assert_eq!(tip.title, "Feb");
    assert_eq!(tip.rows[0].1, "5");
    assert_eq!(tip.points[0].1, feb);
    let rule = tip.rule.expect("a rule");
    assert_eq!((rule.y, rule.h, rule.w), (feb[1].round(), 1.0, plan.plot.w));
}

#[test]
fn text_breaks_its_lines_and_is_cut_as_written() {
    let text = |properties: Properties| {
        let plan = laid_out(
            &mark(Mark::Text)
                .text(Encode::Value("a, b,c".into()))
                .properties(properties),
        );
        let Marks::Texts(texts, font) = &plan.series[0].marks else {
            panic!()
        };
        (texts[0].text.clone(), font.clone())
    };
    let (broken, _) = text(Properties {
        line_break: Some(LineBreak::Text(",".into())),
        ..Properties::default()
    });
    assert_eq!(broken, "a\n b\nc");
    let (broken, font) = text(Properties {
        line_break: Some(LineBreak::Pattern {
            source: ",\\s*".into(),
            flags: "".into(),
        }),
        ellipsis: Some("..".into()),
        dir: Dir::Rtl,
        ..Properties::default()
    });
    assert_eq!(broken, "a\nb\nc");
    assert_eq!((font.ellipsis.as_ref(), font.dir), ("..", Dir::Rtl));
    assert_eq!(text(Properties::default()).1.ellipsis, "\u{2026}");

    // gpui fills a glyph and cannot outline it.
    let labelled = mark(Mark::Text).text(Encode::Value("a".into()));
    assert!(plan_of(&labelled.clone().stroke(Encode::Value(gpui::black()))).is_none());
    assert!(
        plan_of(
            &labelled
                .clone()
                .stroke(Encode::Value(gpui::black()))
                .stroke_width(Encode::Value(0.0))
        )
        .is_some()
    );
}

#[test]
fn slices_turn_round_and_stand_where_their_offsets_say() {
    let data = Data::new().text("kind", ["a", "b"]).number("n", [1.0, 3.0]);
    let pie = |properties: Properties| {
        let plan = laid_out(
            &Chart::arc(data.clone())
                .theta(Channel::quantitative("n"))
                .color(Channel::nominal("kind"))
                .x(Encode::Value(Position::Pixels(100.0)))
                .properties(properties),
        );
        let Marks::Slices(slices) = &plan.series[0].marks else {
            panic!()
        };
        slices[0]
    };
    let plain = pie(Properties::default());
    let turned = pie(Properties {
        theta_offset: 0.5,
        radius_offset: -10.0,
        corner_radius: [Some(4.0); 4],
        pad_angle: 0.1,
        ..Properties::default()
    });
    // Vega-Lite's 5 pixels of padding, then the position.
    assert_eq!(plain.center[0], 5.0 + 100.0);
    assert_eq!(
        (turned.start, turned.end),
        (plain.start + 0.5, plain.end + 0.5)
    );
    assert_eq!(turned.radius, plain.radius - 10.0);
    assert_eq!((turned.corner, turned.pad), (4.0, 0.1));
}

#[test]
fn pies_placed_on_scales_stand_at_their_rows_places() {
    let data = Data::new()
        .text("site", ["a", "a", "b", "b"])
        .text("kind", ["x", "y", "x", "y"])
        .number("n", [1.0, 3.0, 2.0, 2.0])
        .number("height", [10.0, 10.0, 20.0, 20.0]);
    let pies = Chart::arc(data.clone())
        .theta(Channel::quantitative("n"))
        .color(Channel::nominal("kind"))
        .x(Channel::nominal("site"))
        .y(Channel::quantitative("height"))
        .radius(Encode::Value(20.0))
        .layer(
            Chart::new(Mark::Text, data)
                .theta(Channel::quantitative("n"))
                .x(Channel::nominal("site"))
                .y(Channel::quantitative("height"))
                .text(Channel::nominal("kind"))
                .radius(Encode::Value(30.0)),
        );
    let plan = laid_out(&pies);
    assert_eq!(plan.axes.len(), 2);
    let slices: Vec<chart::plan::Slice> = plan
        .series
        .iter()
        .flat_map(|series| match &series.marks {
            Marks::Slices(slices) => slices.clone(),
            _ => Vec::new(),
        })
        .collect();
    // Each site's rows make its own pie, at its band's middle and its
    // height up the values.
    let centers: Vec<[f32; 2]> = slices.iter().map(|slice| slice.center).collect();
    let (a, b) = (centers[0], centers[1]);
    assert!(a[0] < b[0] && a[1] > b[1]);
    assert!(centers.iter().all(|c| *c == a || *c == b));
    let turn = |center: [f32; 2]| -> f32 {
        slices
            .iter()
            .filter(|slice| slice.center == center)
            .map(|slice| slice.end - slice.start)
            .sum()
    };
    let tau = std::f32::consts::TAU;
    assert!((turn(a) - tau).abs() < 1e-4 && (turn(b) - tau).abs() < 1e-4);
    assert_eq!(slices[0].radius, 20.0);
    let Some(Marks::Texts(texts, _)) = plan
        .series
        .iter()
        .find(|series| matches!(series.marks, Marks::Texts(..)))
        .map(|series| &series.marks)
    else {
        panic!()
    };
    assert_eq!(texts.len(), 4);
    let tip = plan.hit([a[0] + 2.0, a[1] - 10.0]).expect("a slice");
    assert_eq!(tip.title, "x");
    assert!(plan.hit([a[0], a[1] - 25.0]).is_none());

    // Without a radius, a pie takes half the plot's shorter side.
    let wide = laid_out(
        &Chart::arc(Data::new().number("n", [1.0]).number("at", [1.0]))
            .theta(Channel::quantitative("n"))
            .x(Channel::quantitative("at"))
            .y(Channel::quantitative("at")),
    );
    let Marks::Slices(slices) = &wide.series[0].marks else {
        panic!()
    };
    assert_eq!(slices[0].radius, wide.plot.w.min(wide.plot.h) / 2.0);
}

#[test]
fn marks_are_cut_off_at_the_plot_only_with_clip() {
    let fixed = |clip: bool| {
        laid_out(
            &Chart::line(xy())
                .x(Channel::quantitative("x"))
                .y(Channel::quantitative("y").domain(2.0, 4.0))
                .properties(Properties {
                    clip,
                    ..Properties::default()
                }),
        )
    };
    // A fixed domain leaves the line past the plot, drawn whole.
    assert_eq!(fixed(false).series[0].clip, None);
    let clipped = fixed(true);
    assert_eq!(clipped.series[0].clip, Some(clipped.plot));
}

/// `hex` as a fixed ink.
fn fixed(hex: u32) -> Ink {
    Ink::Fixed(gpui::rgb(hex).into())
}

/// A colour d3 hands on, as a fixed ink.
fn ink(rgb: Rgb) -> Ink {
    let channel = |v: f64| (v / 255.0) as f32;
    Ink::Fixed(Hsla::from(Rgba {
        r: channel(rgb.r),
        g: channel(rgb.g),
        b: channel(rgb.b),
        a: 1.0,
    }))
}

fn rects(plan: &chart::Plan) -> Vec<Rect> {
    plan.series
        .iter()
        .flat_map(|s| match &s.marks {
            Marks::Bars(bars) => bars.iter().map(|bar| bar.rect).collect(),
            _ => Vec::new(),
        })
        .collect()
}

fn grid() -> Data {
    Data::new()
        .text("a", ["p", "p", "q", "q"])
        .text("b", ["u", "v", "u", "v"])
        .number("n", [1.0, 2.0, 3.0, 4.0])
}

#[test]
fn a_heatmap_fills_the_cells_of_two_discrete_axes() {
    let heatmap = |mark: Mark| {
        laid_out(
            &Chart::new(mark, grid())
                .x(Channel::nominal("a"))
                .y(Channel::nominal("b"))
                .color(Channel::quantitative("n")),
        )
    };
    let plan = heatmap(Mark::Rect);
    assert_eq!(left_labels(&plan), ["u", "v"]);
    let cells = rects(&plan);
    assert_eq!(cells.len(), 4);
    for cell in &cells {
        assert!((cell.w - plan.plot.w / 2.0).abs() < 0.01);
        assert!((cell.h - plan.plot.h / 2.0).abs() < 0.01);
    }
    // A rect's quantitative colour runs through Vega's heatmap scheme from
    // the least value to the most.
    let first = &plan.series[0];
    assert_eq!(plan.colors[first.color].ink, fixed(0xeff9bd));
    let last = plan.series.last().unwrap();
    assert_eq!(plan.colors[last.color].ink, fixed(0x1c3185));
    // Its legend is a gradient, labelled at ticks across the domain.
    let legend = &plan.legends[0];
    assert!(legend.gradient.is_some());
    assert_eq!(legend.labels.first().unwrap().text, "1");
    assert_eq!(legend.labels.last().unwrap().text, "4");

    // Text stands at each cell's middle.
    let plan = laid_out(
        &Chart::new(Mark::Text, grid())
            .x(Channel::nominal("a"))
            .y(Channel::nominal("b"))
            .text(Channel::quantitative("n")),
    );
    let Marks::Texts(texts, _) = &plan.series[0].marks else {
        panic!()
    };
    let (w, h) = (plan.plot.w / 2.0, plan.plot.h / 2.0);
    assert!((texts[0].at[0] - (plan.plot.x + w / 2.0)).abs() < 0.01);
    assert!((texts[0].at[1] - (plan.plot.y + h / 2.0)).abs() < 0.01);
    // A bar over two discrete axes fills its cell too.
    assert_eq!(rects(&heatmap(Mark::Bar)).len(), 4);
}

#[test]
fn one_position_draws_along_its_own_axis() {
    // Ticks along x alone stand across the plot's middle.
    let plan = laid_out(&Chart::new(Mark::Tick, xy()).x(Channel::quantitative("x")));
    assert_eq!(plan.axes.len(), 1);
    assert_eq!(plan.axes[0].side, Side::Bottom);
    let ticks = rects(&plan);
    assert_eq!(ticks.len(), 3);
    let middle = plan.plot.y + plan.plot.h / 2.0;
    assert!((ticks[0].y + ticks[0].h / 2.0 - middle).abs() < 0.01);
    assert!(ticks[0].x < ticks[1].x && ticks[1].x < ticks[2].x);

    // Points down y alone stand at the plot's middle across.
    let plan = laid_out(&Chart::point(xy()).y(Channel::quantitative("y")));
    assert_eq!(plan.axes.len(), 1);
    assert_eq!(plan.axes[0].side, Side::Left);
    let Marks::Points(points) = &plan.series[0].marks else {
        panic!()
    };
    assert!(
        points
            .iter()
            .all(|p| (p.at[0] - (plan.plot.x + plan.plot.w / 2.0)).abs() < 0.01)
    );

    // Points along a nominal x alone, at their bands' middles.
    let plan = laid_out(&Chart::point(xy()).x(Channel::nominal("month")));
    let bottom = &plan.axes[0];
    let labels: Vec<_> = bottom.labels.iter().map(|l| l.text.to_string()).collect();
    assert_eq!(labels, ["Jan", "Feb", "Mar"]);

    // Bars along x alone stack across the plot's full height.
    let plan = laid_out(
        &Chart::bar(xy())
            .x(Channel::quantitative("y"))
            .color(Channel::nominal("month")),
    );
    let bars = rects(&plan);
    assert_eq!(bars.len(), 3);
    assert!(bars.iter().all(|bar| bar.h == plan.plot.h));
    let mut bars = bars;
    bars.sort_by(|a, b| a.x.total_cmp(&b.x));
    assert!((bars[0].right() - bars[1].x).abs() < 0.01);
}

#[test]
fn colours_take_the_spec_s_scheme_or_range_and_the_theme_s_otherwise() {
    let colored = |color: Channel| {
        laid_out(
            &Chart::bar(sales())
                .x(Channel::nominal("month"))
                .y(Channel::quantitative("sales"))
                .color(color),
        )
    };
    let inks = |plan: &chart::Plan| -> Vec<Ink> { plan.colors.iter().map(|c| c.ink).collect() };
    // Nominal with no range: the theme's categorical colours.
    assert_eq!(
        inks(&colored(Channel::nominal("region"))),
        [Ink::Category(0), Ink::Category(1)]
    );
    let scheme = |name: &str| -> Scale {
        Scale {
            scheme: Some(name.parse::<Scheme>().unwrap().into()),
            ..Scale::default()
        }
    };
    assert_eq!(
        inks(&colored(
            Channel::nominal("region").scale(scheme("category10"))
        )),
        [fixed(0x1f77b4), fixed(0xff7f0e)]
    );
    let range = Scale {
        range: Some(Range::Colors(vec![
            gpui::rgb(0xff0000).into(),
            gpui::rgb(0x0000ff).into(),
        ])),
        reverse: Some(true),
        ..Scale::default()
    };
    assert_eq!(
        inks(&colored(Channel::nominal("region").scale(range))),
        [fixed(0x0000ff), fixed(0xff0000)]
    );
    // Ordinal with no range: Vega's blues, sampled once per value.
    let blues: Scheme = "blues".parse().unwrap();
    assert_eq!(
        inks(&colored(Channel::ordinal("region"))),
        blues
            .discrete(2, None, false)
            .into_iter()
            .map(ink)
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_quantitative_colour_runs_through_a_ramp() {
    let plan = laid_out(&mark(Mark::Point).color(Channel::quantitative("y")));
    // y spans 2 to 5: the ends of Vega's blues.
    let color_of = |row: f32| {
        plan.colors
            .iter()
            .find(|c| c.name.as_deref() == Some(&*row.to_string()))
            .unwrap()
            .ink
    };
    assert_eq!(color_of(2.0), fixed(0xcfe1f2));
    assert_eq!(color_of(5.0), fixed(0x0a4a90));
    // A diverging scale centres on its middle.
    let diverging = laid_out(
        &mark(Mark::Point).color(Channel::quantitative("y").scale(Scale {
            domain_mid: Some(3.0),
            ..Scale::default()
        })),
    );
    let middle = diverging
        .colors
        .iter()
        .find(|c| c.name.as_deref() == Some("3"))
        .unwrap()
        .ink;
    let blue_orange: Scheme = "blueorange".parse().unwrap();
    assert_eq!(middle, ink(blue_orange.at(0.5, Some([1.0, 0.0]), false)));
    // Quantize steps through the scheme's samples.
    let quantized = laid_out(
        &mark(Mark::Point).color(Channel::quantitative("y").scale(Scale {
            kind: Some(ScaleType::Quantize),
            ..Scale::default()
        })),
    );
    // A discrete gradient, one cell to each step.
    let gradient = quantized.legends[0].gradient.as_ref().unwrap();
    let Fill::Bands(bands) = &gradient.fill else {
        panic!()
    };
    assert_eq!(bands.len(), 5);
}

#[test]
fn stacks_offset_as_their_channel_says() {
    let stacked = |stack: Stack| {
        laid_out(
            &Chart::bar(sales())
                .x(Channel::nominal("month"))
                .y(Channel::quantitative("sales").stack(stack))
                .color(Channel::nominal("region")),
        )
    };
    // Normalized, every stack reaches the top.
    let plan = stacked(Stack::Normalize);
    let tops: Vec<f32> = rects(&plan).iter().map(|r| r.y).collect();
    let top = tops.iter().copied().fold(f32::INFINITY, f32::min);
    assert!((top - plan.plot.y).abs() < 0.01);
    assert_eq!(left_labels(&plan).last().unwrap(), "1.0");
    // Centred, each stack's middle is the largest one's.
    let plan = stacked(Stack::Center);
    let bars = rects(&plan);
    let middle = |x: f32| {
        let column: Vec<&Rect> = bars.iter().filter(|r| (r.x - x).abs() < 0.5).collect();
        let lo = column.iter().map(|r| r.y).fold(f32::INFINITY, f32::min);
        let hi = column
            .iter()
            .map(|r| r.bottom())
            .fold(f32::NEG_INFINITY, f32::max);
        (lo + hi) / 2.0
    };
    let xs: Vec<f32> = bars.iter().map(|r| r.x).collect();
    assert!((middle(xs[0]) - middle(xs[1])).abs() < 0.01);
    // Off, each bar stands from zero, a negative one hanging from it.
    let plan = stacked(Stack::Off);
    let bars = rects(&plan);
    let zero = bars[0].bottom();
    assert_eq!(
        bars.iter()
            .filter(|r| (r.bottom() - zero).abs() < 0.01)
            .count(),
        4
    );
    assert_eq!(bars.iter().filter(|r| (r.y - zero).abs() < 0.01).count(), 1);

    // Areas stack, a series with no row at a place standing at zero there.
    let data = Data::new()
        .number("x", [0.0, 1.0, 2.0, 0.0, 2.0])
        .number("y", [1.0, 1.0, 1.0, 2.0, 2.0])
        .text("s", ["a", "a", "a", "b", "b"]);
    let plan = laid_out(
        &Chart::area(data)
            .x(Channel::quantitative("x"))
            .y(Channel::quantitative("y"))
            .color(Channel::nominal("s")),
    );
    let Marks::Areas(b) = &plan.series[1].marks else {
        panic!()
    };
    assert_eq!(b[0].top.len(), 3);
}

#[test]
fn scale_types_and_properties_place_positions() {
    let data = Data::new()
        .number("x", [1.0, 10.0, 100.0, 1000.0])
        .number("y", [1.0, 2.0, 3.0, 4.0]);
    let plan = laid_out(
        &Chart::point(data)
            .x(Channel::quantitative("x").scale(Scale {
                kind: Some(ScaleType::Log),
                ..Scale::default()
            }))
            .y(Channel::quantitative("y")),
    );
    let Marks::Points(points) = &plan.series[0].marks else {
        panic!()
    };
    let gaps: Vec<f32> = points.windows(2).map(|p| p[1].at[0] - p[0].at[0]).collect();
    assert!(gaps.iter().all(|gap| (gap - gaps[0]).abs() < 0.01));
    // Each power of ten and its multiples in between.
    let bottom = plan.axes.iter().find(|a| a.side == Side::Bottom).unwrap();
    // The domain line, then each tick.
    assert_eq!(bottom.rules.len() - 1, 28);

    // A reversed band scale puts the last value first; padding thins bars.
    let plan = laid_out(
        &Chart::bar(sales())
            .x(Channel::nominal("month").scale(Scale {
                reverse: Some(true),
                padding_inner: Some(0.5),
                ..Scale::default()
            }))
            .y(Channel::quantitative("sales")),
    );
    let bottom = plan.axes.iter().find(|a| a.side == Side::Bottom).unwrap();
    let first = bottom.rules[1].from[0];
    assert!(first > bottom.rules[3].from[0]);
    let bar = rects(&plan)[0];
    // Three bands with half of each step empty, a quarter step outside.
    let step = plan.plot.w / (3.0 - 0.5 + 0.5);
    assert!((bar.w - step * 0.5).abs() < 0.01);
}

#[test]
fn colours_come_unscaled_from_a_field_or_its_range_field() {
    let data = Data::new()
        .text("name", ["red", "blue"])
        .text("tone", ["#00ff00", "#0000ff"])
        .number("v", [1.0, 2.0]);
    let inks = |color: Channel| -> Vec<Ink> {
        let plan = laid_out(
            &Chart::bar(data.clone())
                .x(Channel::nominal("name"))
                .y(Channel::quantitative("v"))
                .color(color),
        );
        plan.colors.iter().map(|c| c.ink).collect()
    };
    let identity = Scale {
        kind: Some(ScaleType::Identity),
        ..Scale::default()
    };
    assert_eq!(
        inks(Channel::nominal("name").scale(identity)),
        [fixed(0xff0000), fixed(0x0000ff)]
    );
    let field = Scale {
        range: Some(Range::Field("tone".into())),
        ..Scale::default()
    };
    assert_eq!(
        inks(Channel::nominal("name").scale(field)),
        [fixed(0x00ff00), fixed(0x0000ff)]
    );
}

#[test]
fn domain_and_range_ends_move_as_written() {
    let plan = laid_out(
        &mark(Mark::Point).y(Channel::quantitative("y").scale(Scale {
            domain_min: Some(-10.0),
            domain_max: Some(10.0),
            ..Scale::default()
        })),
    );
    // No nice past the written ends: 0 sits in the middle.
    let Marks::Points(points) = &plan.series[0].marks else {
        panic!()
    };
    let middle = plan.plot.y + plan.plot.h / 2.0;
    let five = middle - plan.plot.h / 2.0 * 0.5;
    assert!((points[1].at[1] - five).abs() < 0.01);

    // A size range's ends.
    let plan = laid_out(
        &mark(Mark::Point).size(Channel::quantitative("y").scale(Scale {
            range_min: Some(100.0),
            range_max: Some(200.0),
            zero: Some(false),
            ..Scale::default()
        })),
    );
    let Marks::Points(points) = &plan.series[0].marks else {
        panic!()
    };
    assert_eq!(points[0].size, 100.0);
    assert_eq!(points[1].size, 200.0);

    // x from 20 pixels into the plot.
    let plan = laid_out(
        &mark(Mark::Point).x(Channel::quantitative("x").scale(Scale {
            range_min: Some(20.0),
            zero: Some(false),
            nice: Some(Nice::Off),
            ..Scale::default()
        })),
    );
    let Marks::Points(points) = &plan.series[0].marks else {
        panic!()
    };
    assert!((points[0].at[0] - (plan.plot.x + 20.0)).abs() < 0.01);
}

#[test]
fn a_time_domain_is_nice_to_an_interval() {
    let days: Vec<f64> = [3.0, 40.0]
        .iter()
        .map(|d| time::parse("2024-01-01").unwrap() + d * time::DAY)
        .collect();
    let data = Data::new().number("t", days).number("v", [1.0, 2.0]);
    let plan = laid_out(
        &Chart::point(data)
            .x(Channel::temporal("t").scale(Scale {
                nice: Some(Nice::Interval {
                    unit: Unit::Month,
                    step: 1.0,
                }),
                ..Scale::default()
            }))
            .y(Channel::quantitative("v")),
    );
    // Jan 1 to Mar 1: the first point a 20th of the way along.
    let Marks::Points(points) = &plan.series[0].marks else {
        panic!()
    };
    let share = (points[0].at[0] - plan.plot.x) / plan.plot.w;
    assert!((share - 3.0 / 60.0).abs() < 0.001);
}

#[test]
fn a_scheme_of_colours_is_a_ramp() {
    let scale = Scale {
        scheme: Some(ColorScheme {
            palette: Palette::Colors(vec![gpui::rgb(0x000000).into(), gpui::rgb(0xffffff).into()]),
            extent: None,
            count: None,
        }),
        interpolate: Some(interpolate::Interpolate::Rgb { gamma: 1.0 }),
        ..Scale::default()
    };
    let plan = laid_out(&mark(Mark::Point).color(Channel::quantitative("y").scale(scale)));
    let ink = |name: &str| {
        plan.colors
            .iter()
            .find(|c| c.name.as_deref() == Some(name))
            .unwrap()
            .ink
    };
    assert_eq!(ink("2"), fixed(0x000000));
    assert_eq!(ink("5"), fixed(0xffffff));
}

#[test]
fn a_path_symbol_is_drawn_from_its_path() {
    let star: Shape = "M0,-1L1,1L-1,1Z".parse().unwrap();
    let outlines = star.outlines(16.0, 0.0);
    assert_eq!(outlines.len(), 1);
    // Half the square root of the size: 2 pixels a unit.
    assert_eq!(outlines[0][0], [0.0, -2.0]);
    assert!("not a path".parse::<Shape>().is_err());
}

#[test]
fn bars_along_a_continuous_axis_are_thin() {
    let plan = laid_out(&mark(Mark::Bar));
    let bars = rects(&plan);
    assert_eq!(bars.len(), 3);
    assert!(bars.iter().all(|bar| (bar.w - 5.0).abs() < 0.01));
    // y alone with a horizontal orient: thin bars across the plot.
    let plan = laid_out(
        &Chart::bar(xy())
            .y(Channel::quantitative("y"))
            .properties(Properties {
                orient: Some(Orient::Horizontal),
                ..Properties::default()
            }),
    );
    let bars = rects(&plan);
    assert!(
        bars.iter()
            .all(|bar| bar.w == plan.plot.w && (bar.h - 5.0).abs() < 0.01)
    );
}

/// A chart over `t` and `v`, the last row's `v` missing and the third's
/// `s`, drawn as `mark` in `mode`, about 3 ticks along `t`.
fn invalid(mark: Mark, mode: Option<Invalid>) -> chart::Plan {
    let data = Data::new()
        .number("t", [1.0, 2.0, 3.0, 4.0, 10.0])
        .number("v", [1.0, 2.0, 3.0, 4.0, f64::NAN])
        .number("s", [1.0, 2.0, f64::NAN, 2.0, 1.0]);
    let axis = Axis {
        tick_count: Some(TickCount::Count(3.0)),
        ..Axis::default()
    };
    let chart = Chart::new(mark, data)
        .x(Channel::quantitative("t").axis(Some(axis)))
        .y(Channel::quantitative("v"))
        .properties(Properties {
            invalid: mode,
            ..Properties::default()
        });
    laid_out(&chart)
}

/// The last label along the bottom.
fn last_x(plan: &chart::Plan) -> String {
    let axis = plan
        .axes
        .iter()
        .find(|axis| axis.side == Side::Bottom)
        .unwrap();
    axis.labels.last().unwrap().text.to_string()
}

#[test]
fn invalid_rows_leave_the_domains_a_mode_filters() {
    // Vega's x domains: [1, 10] where the domains take the row in, [1, 4]
    // for a line and [0, 4] for points where they leave it out.
    assert_eq!(last_x(&invalid(Mark::Line, None)), "10");
    assert_eq!(last_x(&invalid(Mark::Line, Some(Invalid::Filter))), "4");
    let filtered = invalid(Mark::Line, Some(Invalid::BreakPathsFilterDomains));
    assert_eq!(last_x(&filtered), "4");
    assert_eq!(
        filtered.chart().layers[0].data.len(),
        5,
        "the line keeps the row"
    );
    assert_eq!(last_x(&invalid(Mark::Point, None)), "4");
    let shown = invalid(Mark::Point, Some(Invalid::BreakPathsShowDomains));
    assert_eq!(last_x(&shown), "10");
    assert_eq!(shown.chart().layers[0].data.len(), 4, "no point is drawn");
    let Marks::Points(points) = &shown.series[0].marks else {
        panic!("points");
    };
    assert_eq!(points.len(), 4);
}

#[test]
fn a_path_breaks_at_an_invalid_value_of_any_channel_that_counts() {
    let trail = |mode| {
        let data = Data::new()
            .number("t", [1.0, 2.0, 3.0, 4.0])
            .number("v", [1.0, 2.0, 3.0, 4.0])
            .number("s", [1.0, 2.0, f64::NAN, 2.0]);
        let chart = Chart::new(Mark::Trail, data)
            .x(Channel::quantitative("t"))
            .y(Channel::quantitative("v"))
            .size(Channel::quantitative("s"))
            .properties(Properties {
                invalid: mode,
                ..Properties::default()
            });
        let Marks::Trails(runs) = laid_out(&chart).series[0].marks.clone() else {
            panic!("trails");
        };
        runs.iter().map(Vec::len).collect::<Vec<_>>()
    };
    assert_eq!(trail(None), [2, 1]);
    assert_eq!(trail(Some(Invalid::Filter)), [3]);
}

#[test]
fn shown_invalid_values_stand_at_zero_or_the_domains_least() {
    let shown = |values: [f64; 4], zero: bool| {
        let data = Data::new()
            .text("k", ["a", "b", "c", "d"])
            .number("v", values);
        let chart = Chart::point(data)
            .x(Channel::nominal("k"))
            .y(Channel::quantitative("v").zero(zero))
            .properties(Properties {
                invalid: Some(Invalid::Show),
                ..Properties::default()
            });
        let Marks::Points(points) = laid_out(&chart).series[0].marks.clone() else {
            panic!("points");
        };
        points.iter().map(|point| point.at[1]).collect::<Vec<_>>()
    };
    // At zero where the domain spans it.
    let at = shown([-5.0, 10.0, 0.0, f64::NAN], true);
    assert_eq!(at.len(), 4);
    assert_eq!(at[3], at[2]);
    // At the domain's least where it does not.
    let at = shown([5.0, 10.0, 7.0, f64::NAN], false);
    assert_eq!(at[3], at[0]);
}

#[test]
fn shown_invalid_colours_take_a_discretizing_scales_zero_or_least() {
    let shown = |kind: ScaleType, values: [f64; 4]| {
        let data = Data::new()
            .text("k", ["a", "b", "c", "d"])
            .number("v", values);
        let chart = Chart::point(data)
            .x(Channel::nominal("k"))
            .color(Channel::quantitative("v").scale(Scale {
                kind: Some(kind),
                ..Scale::default()
            }))
            .properties(Properties {
                invalid: Some(Invalid::Show),
                ..Properties::default()
            });
        // Each point's style, left to right.
        let mut points: Vec<Symbol> = laid_out(&chart)
            .series
            .into_iter()
            .flat_map(|series| match series.marks {
                Marks::Points(points) => points,
                _ => panic!("points"),
            })
            .collect();
        points.sort_by(|a, b| a.at[0].total_cmp(&b.at[0]));
        points
            .into_iter()
            .map(|point| point.style)
            .collect::<Vec<_>>()
    };
    for kind in [ScaleType::Quantile, ScaleType::Quantize] {
        // The least where the domain does not span zero.
        let styles = shown(kind, [5.0, 10.0, 7.0, f64::NAN]);
        assert_eq!(styles.len(), 4, "{kind:?}");
        assert_eq!(styles[3], styles[0], "{kind:?}");
        assert_ne!(styles[3], styles[1], "{kind:?}");
        // Zero where it does.
        let styles = shown(kind, [-5.0, 10.0, 0.0, f64::NAN]);
        assert_eq!(styles[3], styles[2], "{kind:?}");
        assert_ne!(styles[3], styles[1], "{kind:?}");
    }
}
