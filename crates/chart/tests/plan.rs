use chart::{
    Channel, Chart, Data, Sort,
    decimate::m4,
    plan::{Marks, Side, plan},
    scale, time,
};

const SIZE: [f32; 2] = [400.0, 300.0];
const LINE: f32 = 14.0;

fn measure(text: &str) -> f32 {
    text.chars().count() as f32 * 7.0
}

fn laid_out(chart: &Chart) -> chart::Plan {
    plan(chart, SIZE, LINE, &mut measure).expect("plans")
}

fn sales() -> Data {
    Data::new()
        .text("month", ["Jan", "Jan", "Feb", "Feb", "Mar"])
        .text("region", ["east", "west", "east", "west", "east"])
        .number("sales", [3.0, 2.0, 5.0, -1.0, 4.0])
}

#[test]
fn linear_ticks_are_round_and_cover_the_data() {
    let ticks = scale::linear(0.0, 97.0, 5, true);
    assert_eq!(ticks.values, [0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
    assert_eq!(ticks.domain, (0.0, 100.0));
    assert_eq!(
        scale::linear(0.0, 0.3, 3, true).labels,
        ["0.0", "0.1", "0.2", "0.3"]
    );
    assert_eq!(
        scale::linear(0.0, 25_000.0, 2, true).labels,
        ["0", "10k", "20k", "30k"]
    );
    assert_eq!(
        scale::linear(0.0, 4_000.0, 2, true).labels,
        ["0", "2,000", "4,000"]
    );
}

#[test]
fn values_read_out_whole() {
    assert_eq!(scale::value(1234567.0), "1,234,567");
    assert_eq!(scale::value(0.1 + 0.2), "0.3");
    assert_eq!(scale::value(-12.345), "-12.35");
    assert_eq!(scale::value(0.001234), "0.00123");
}

#[test]
fn time_round_trips_the_calendar() {
    for days in [-800_000, -1, 0, 59, 10_957, 19_782, 2_000_000] {
        let (y, m, d) = time::civil(days);
        assert_eq!(time::days(y, m, d), days);
    }
    let at = time::parse("2024-03-05T14:30:00Z").unwrap();
    assert_eq!(time::describe(at), "2024-03-05 14:30");
    assert_eq!(
        time::describe(time::parse("2024-03-05").unwrap()),
        "2024-03-05"
    );
    assert!(time::parse("2024-03-05T14:30+02:00").is_none());
}

#[test]
fn time_ticks_land_on_calendar_boundaries() {
    let lo = time::parse("2024-01-10").unwrap();
    let hi = time::parse("2024-12-20").unwrap();
    let ticks = scale::temporal(lo, hi, 4);
    assert_eq!(ticks.labels, ["Apr", "Jul", "Oct"]);

    let (lo, hi) = (
        time::parse("2020-06-01").unwrap(),
        time::parse("2024-06-01").unwrap(),
    );
    assert_eq!(
        scale::temporal(lo, hi, 4).labels,
        ["2021", "2022", "2023", "2024"]
    );

    let lo = time::parse("2024-03-05T09:10").unwrap();
    let ticks = scale::temporal(lo, lo + 3.0 * time::HOUR, 3);
    assert_eq!(ticks.labels, ["10:00", "11:00", "12:00"]);
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
    assert_eq!(plan.legend.len(), 2);
    let bars = |s: usize| match &plan.series[s].marks {
        Marks::Bars(bars) => bars.clone(),
        other => panic!("bars, not {other:?}"),
    };
    let (east, west) = (bars(0), bars(1));
    assert_eq!((east.len(), west.len()), (3, 2));
    // Jan: west sits on east.
    assert!((west[0].bottom() - east[0].y).abs() < 0.01);
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
    let feb = east[1];
    let tip = plan.hit([feb.x + 1.0, plan.plot.y + 1.0]).unwrap();
    assert_eq!(tip.title, "Feb");
    assert_eq!(tip.rows, [(0, "5".into()), (1, "-1".into())]);
    assert!(plan.hit([0.0, 0.0]).is_none());
}

#[test]
fn horizontal_bars_put_categories_on_the_left() {
    let chart = Chart::bar(sales())
        .y(Channel::ordinal("month"))
        .x(Channel::quantitative("sales"));
    let plan = laid_out(&chart);
    let left = plan.axes.iter().find(|a| a.side == Side::Left).unwrap();
    let labels: Vec<_> = left
        .ticks
        .iter()
        .filter_map(|t| t.label.as_ref())
        .map(|l| l.text.as_ref())
        .collect();
    assert_eq!(labels, ["Jan", "Feb", "Mar"]);
    assert!(!left.grid);
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
    let outline = &areas[0];
    assert_eq!(outline.len(), 5);
    assert_eq!(outline[3][1], plan.plot.bottom());
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
    assert_eq!(tip.rows[0].1, "2 (25.0%)");
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
    assert!(
        bottom
            .ticks
            .iter()
            .filter_map(|t| t.label.as_ref())
            .any(|l| l.text == "Feb")
    );
    let tip = plan.hit([plan.plot.x + 1.0, plan.plot.y + 1.0]).unwrap();
    assert_eq!(tip.title, "2024-01-01");
}

#[test]
fn an_encoding_the_mark_cannot_draw_is_none() {
    let chart = |c: Chart| plan(&c, SIZE, LINE, &mut measure).is_none();
    assert!(chart(
        Chart::bar(sales())
            .x(Channel::nominal("month"))
            .y(Channel::nominal("region"))
    ));
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
        Chart::arc(sales())
            .theta(Channel::quantitative("sales"))
            .color(Channel::quantitative("sales"))
    ));
    let tiny = Chart::bar(sales())
        .x(Channel::nominal("month"))
        .y(Channel::quantitative("sales"));
    assert!(plan(&tiny, [20.0, 20.0], LINE, &mut measure).is_none());
}

fn temperatures() -> Data {
    Data::new()
        .number("x", [0.0, 1.0, 2.0])
        .number("t", [21.0, 24.0, 22.5])
}

fn left_labels(plan: &chart::Plan) -> Vec<String> {
    let left = plan.axes.iter().find(|a| a.side == Side::Left).unwrap();
    left.ticks
        .iter()
        .filter_map(|t| t.label.as_ref())
        .map(|l| l.text.to_string())
        .collect()
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
    assert_eq!(plan.legend_title.as_ref().unwrap().text, "Region");
    assert!(plan.legend[0].swatch.x > plan.legend_title.as_ref().unwrap().width);
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
    let months: Vec<_> = bottom
        .ticks
        .iter()
        .filter_map(|t| t.label.as_ref())
        .map(|l| l.text.as_ref())
        .collect();
    assert_eq!(months, ["Jan", "Feb", "Mar", "Apr"]);
    // The line's 10 sets the top, which the bars are measured against.
    assert_eq!(left_labels(&plan).last().unwrap(), "10");
    let colors: Vec<usize> = plan.series.iter().map(|s| s.color).collect();
    assert_eq!(colors, [0, 1]);
    assert!(plan.legend.is_empty());

    // Over Feb: the bar's band and the line's point read out together.
    let Marks::Bars(bars) = &plan.series[0].marks else {
        panic!()
    };
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
    assert_eq!(plan.colors, [Some("east".into()), Some("west".into())]);
    let line = plan
        .series
        .iter()
        .find(|s| s.mark == chart::Mark::Line)
        .unwrap();
    assert_eq!(line.color, 1);
}

#[test]
fn layers_that_disagree_or_layer_an_arc_are_none() {
    let line = Chart::line(temperatures())
        .x(Channel::quantitative("x"))
        .y(Channel::quantitative("t"));
    let bars = Chart::bar(sales())
        .x(Channel::nominal("month"))
        .y(Channel::quantitative("sales"));
    assert!(plan(&bars.clone().layer(line.clone()), SIZE, LINE, &mut measure).is_none());
    let pie = Chart::arc(sales()).theta(Channel::quantitative("sales"));
    assert!(plan(&pie.layer(line), SIZE, LINE, &mut measure).is_none());
}
