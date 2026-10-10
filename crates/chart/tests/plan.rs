use chart::{
    Chart, Data, Kind,
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
    let ticks = scale::linear(0.0, 97.0, 5);
    assert_eq!(ticks.values, [0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
    assert_eq!(ticks.domain, (0.0, 100.0));
    assert_eq!(
        scale::linear(0.0, 0.3, 3).labels,
        ["0.0", "0.1", "0.2", "0.3"]
    );
    assert_eq!(
        scale::linear(0.0, 25_000.0, 2).labels,
        ["0", "10k", "20k", "30k"]
    );
    assert_eq!(
        scale::linear(0.0, 4_000.0, 2).labels,
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
        .x("month", Kind::Nominal)
        .y("sales", Kind::Quantitative)
        .color("region", Kind::Nominal);
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
        .x("month", Kind::Nominal)
        .y("sales", Kind::Quantitative)
        .color("region", Kind::Nominal);
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
        .y("month", Kind::Ordinal)
        .x("sales", Kind::Quantitative);
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
            .x("t", Kind::Quantitative)
            .y("v", Kind::Quantitative),
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
            .x("x", Kind::Quantitative)
            .y("y", Kind::Quantitative),
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
            .x("x", Kind::Quantitative)
            .y("y", Kind::Quantitative),
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
            .x("x", Kind::Quantitative)
            .y("y", Kind::Quantitative),
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
            .theta("n", Kind::Quantitative)
            .color("kind", Kind::Nominal),
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
            .x("day", Kind::Temporal)
            .y("v", Kind::Quantitative),
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
            .x("month", Kind::Nominal)
            .y("region", Kind::Nominal)
    ));
    assert!(chart(
        Chart::bar(sales())
            .x("month", Kind::Nominal)
            .y("missing", Kind::Quantitative)
    ));
    assert!(chart(
        Chart::line(sales())
            .x("month", Kind::Quantitative)
            .y("sales", Kind::Quantitative)
    ));
    assert!(chart(
        Chart::arc(sales())
            .theta("sales", Kind::Quantitative)
            .color("sales", Kind::Quantitative)
    ));
    let tiny = Chart::bar(sales())
        .x("month", Kind::Nominal)
        .y("sales", Kind::Quantitative);
    assert!(plan(&tiny, [20.0, 20.0], LINE, &mut measure).is_none());
}
