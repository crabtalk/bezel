//! Axes, legends and the title laid out as Vega lays them out.

use chart::{
    Channel, Chart, Data,
    model::{
        Anchor, Axis, Case, Datum, Format, Legend, LegendOrient, Lettering, Line, Orient, Side,
        Test, TickBand, TickCount, TickStyle, Title,
    },
    plan::{Face, Fill, Ink, Metrics, Plan, Rect, plan},
};
use gpui::Hsla;
use std::sync::Arc;

const SIZE: [f32; 2] = [400.0, 300.0];
const METRICS: Metrics = Metrics {
    size: 10.0,
    line: 12.0,
};

/// Every character 6 pixels wide at the host's size, and in proportion at
/// any other.
fn laid_out(chart: &Chart) -> Plan {
    let mut measure = |text: &str, face: &Face| text.chars().count() as f32 * face.size * 0.6;
    plan(chart, SIZE, METRICS, &mut measure).expect("plans")
}

fn data() -> Data {
    Data::new()
        .text("k", ["a", "b", "c", "a"])
        .number("x", [0.0, 25.0, 50.0, 100.0])
        .number("y", [-2.0, 1.0, 3.0, 4.0])
}

fn points() -> Chart {
    Chart::point(data())
        .x(Channel::quantitative("x"))
        .y(Channel::quantitative("y"))
}

fn axis(plan: &Plan, side: Side) -> &chart::plan::Axis {
    plan.axes
        .iter()
        .find(|axis| axis.side == side)
        .expect("an axis on that side")
}

fn texts(labels: &[chart::plan::Label]) -> Vec<String> {
    labels.iter().map(|label| label.text.to_string()).collect()
}

#[test]
fn an_axis_takes_vegas_ticks_labels_and_lines() {
    let plan = laid_out(&points().x(Channel::quantitative("x").title("X")));
    let bottom = axis(&plan, Side::Bottom);
    // One tick to about 40 pixels, written to the precision their step needs.
    assert_eq!(
        texts(&bottom.labels),
        [
            "0", "10", "20", "30", "40", "50", "60", "70", "80", "90", "100"
        ]
    );
    // The domain line along the plot's bottom, then a tick down from each.
    let domain = &bottom.rules[0];
    assert_eq!(domain.from[1], plan.plot.bottom());
    assert_eq!(domain.to[0] - domain.from[0], plan.plot.w);
    assert_eq!(bottom.rules.len(), 12);
    assert_eq!(bottom.rules[1].to[1] - bottom.rules[1].from[1], 5.0);
    assert_eq!(bottom.rules[1].stroke.paint.ink, Ink::Border);
    // A grid line across the plot at each tick of a quantitative field.
    assert_eq!(bottom.grid.len(), 11);
    assert_eq!(bottom.grid[0].stroke.paint.ink, Ink::Faint);
    // The first and last labels flush with the axis's ends.
    let first = &bottom.labels[0];
    let last = bottom.labels.last().unwrap();
    assert_eq!(first.at[0], plan.plot.x);
    assert_eq!(last.at[0] + last.width, plan.plot.right());
    assert_eq!(first.at[1], plan.plot.bottom() + 5.0 + 2.0);
    // The title under the labels, at the middle.
    let title = bottom.title.as_ref().unwrap();
    assert_eq!(
        title.at[0] + title.width / 2.0,
        plan.plot.x + plan.plot.w / 2.0
    );
    assert!(title.at[1] >= first.at[1] + first.height() + 4.0);
    // A negative number is written with a minus sign.
    let left = axis(&plan, Side::Left);
    assert_eq!(left.labels[0].text, "\u{2212}2");
    // The plot takes what the axes leave.
    assert!(left.labels.iter().all(|label| label.at[0] >= 5.0));
    assert!(title.at[1] + title.height() <= SIZE[1] - 5.0);
}

#[test]
fn an_axis_stands_where_its_orient_says_and_draws_what_it_is_told() {
    let x = Axis {
        orient: Some(Side::Top),
        offset: Some(4.0),
        domain: Line {
            show: Some(false),
            ..Line::default()
        },
        style: TickStyle {
            grid: Line {
                show: Some(false),
                ..Line::default()
            },
            tick: Line {
                show: Some(false),
                ..Line::default()
            },
            ..TickStyle::default()
        },
        values: Some(vec![Datum::Number(10.0), Datum::Number(60.0)]),
        format: Some(Format::Number(".1f".parse().unwrap())),
        ..Axis::default()
    };
    let y = Axis {
        orient: Some(Side::Right),
        labels: Some(false),
        tick_count: Some(TickCount::Count(2.0)),
        ..Axis::default()
    };
    let plan = laid_out(
        &Chart::point(data())
            .x(Channel::quantitative("x").axis(Some(x)))
            .y(Channel::quantitative("y").axis(Some(y))),
    );
    let top = axis(&plan, Side::Top);
    assert!(top.rules.is_empty() && top.grid.is_empty());
    assert_eq!(texts(&top.labels), ["10.0", "60.0"]);
    // Without ticks, labels stand their padding out from the offset axis.
    let label = &top.labels[0];
    assert_eq!(label.at[1] + label.height(), plan.plot.y - 4.0 - 2.0);
    let right = axis(&plan, Side::Right);
    assert!(right.labels.is_empty());
    assert_eq!(right.rules[0].from[0], plan.plot.right());
    // About two ticks asked for: d3's step of 2 from -2 to 4.
    assert_eq!(right.rules.len(), 1 + 4);

    // No axis at all.
    let plan = laid_out(&Chart::point(data()).x(Channel::quantitative("x").axis(None)));
    assert!(plan.axes.is_empty());
}

#[test]
fn an_axis_relabels_and_styles_each_tick_as_its_cases_say() {
    let red = Hsla::red();
    let zero = Case {
        test: Test(Arc::new(|value: &Datum, _: &str| {
            *value == Datum::Number(0.0)
        })),
        style: TickStyle {
            grid: Line {
                color: Some(red),
                ..Line::default()
            },
            label: Lettering {
                color: Some(red),
                ..Lettering::default()
            },
            ..TickStyle::default()
        },
    };
    let y = Axis {
        cases: vec![zero],
        relabel: Some(chart::model::Relabel(Arc::new(
            |_: &Datum, label: &str, _| format!("{label}%").into(),
        ))),
        ..Axis::default()
    };
    let plan = laid_out(&points().y(Channel::quantitative("y").axis(Some(y))));
    let left = axis(&plan, Side::Left);
    assert!(left.labels.iter().all(|label| label.text.ends_with('%')));
    let zero = left.labels.iter().position(|l| l.text == "0%").unwrap();
    assert_eq!(left.labels[zero].paint.ink, Ink::Fixed(red));
    assert_eq!(left.grid[zero].stroke.paint.ink, Ink::Fixed(red));
    assert_eq!(left.grid[zero + 1].stroke.paint.ink, Ink::Faint);
}

#[test]
fn a_band_axis_ticks_its_bands() {
    let x = Axis {
        tick_band: Some(TickBand::Extent),
        ..Axis::default()
    };
    let plan = laid_out(
        &Chart::bar(data())
            .x(Channel::nominal("k").axis(Some(x)))
            .y(Channel::quantitative("y")),
    );
    let bottom = axis(&plan, Side::Bottom);
    assert_eq!(texts(&bottom.labels), ["a", "b", "c"]);
    // A tick at each band's end and one at the first's start, and no grid.
    assert_eq!(bottom.rules.len(), 1 + 4);
    assert!(bottom.grid.is_empty());
    // A level y title stands over the axis's top end.
    let plan = laid_out(
        &Chart::bar(data())
            .x(Channel::nominal("k"))
            .y(Channel::quantitative("y").title("Why")),
    );
    let title = axis(&plan, Side::Left).title.as_ref().unwrap();
    assert!(title.at[1] + title.height() <= plan.plot.y);
}

#[test]
fn a_time_axis_writes_vegas_multi_format() {
    let day = 24.0 * 3600e3;
    let start = chart::time::parse("2024-01-01").unwrap();
    let data = Data::new()
        .number(
            "t",
            (0..366).map(|d| start + d as f64 * day).collect::<Vec<_>>(),
        )
        .number("v", vec![1.0; 366]);
    let plan = laid_out(
        &Chart::line(data)
            .x(Channel::temporal("t"))
            .y(Channel::quantitative("v")),
    );
    let labels = texts(&axis(&plan, Side::Bottom).labels);
    assert_eq!(labels[0], "2024");
    // A tick each month, every other label hidden where they overlap.
    assert!(labels.contains(&"March".to_string()));
}

#[test]
fn legends_stand_where_their_orient_says() {
    let legend = |orient: LegendOrient| {
        let spec = Legend {
            orient: Some(orient),
            legend_x: Some(30.0),
            legend_y: Some(40.0),
            ..Legend::default()
        };
        let plan = laid_out(&points().color(Channel::nominal("k").legend(Some(spec))));
        let bounds = plan.legends[0].bounds().unwrap();
        (plan, bounds)
    };
    // Beside what the plot and its axes cover.
    let covered = |plan: &Plan| {
        plan.axes
            .iter()
            .filter_map(|a| a.bounds())
            .fold(plan.plot, Rect::union)
    };
    let (plan, right) = legend(LegendOrient::Right);
    let left = axis(&plan, Side::Left);
    assert_eq!(right.x, covered(&plan).right() + 18.0);
    assert_eq!(right.y, plan.plot.y);
    assert!(right.right() <= SIZE[0] - 5.0);
    assert!(left.labels.iter().all(|l| l.at[0] >= 5.0));

    let (plan, bottom) = legend(LegendOrient::Bottom);
    assert_eq!(bottom.y, covered(&plan).bottom() + 18.0);
    assert_eq!(bottom.x, plan.plot.x);
    // Across a side's run, its entries flow in one row.
    let ys: Vec<f32> = plan.legends[0]
        .swatches
        .iter()
        .map(|s| s.swatch.y)
        .collect();
    assert!(ys.iter().all(|y| *y == ys[0]));

    let (plan, corner) = legend(LegendOrient::TopRight);
    assert_eq!(corner.right(), plan.plot.right() - 18.0);
    assert_eq!(corner.y, plan.plot.y + 18.0);

    let (plan, placed) = legend(LegendOrient::None);
    assert_eq!(
        [placed.x, placed.y],
        [plan.plot.x + 30.0, plan.plot.y + 40.0]
    );

    // A legend of `None` draws none.
    let plan = laid_out(&points().color(Channel::nominal("k").legend(None)));
    assert!(plan.legends.is_empty());
}

#[test]
fn a_symbol_legend_lays_its_entries_in_columns() {
    let spec = Legend {
        direction: Some(Orient::Horizontal),
        columns: Some(2),
        ..Legend::default()
    };
    let plan = laid_out(&points().color(Channel::nominal("k").legend(Some(spec))));
    let swatches = &plan.legends[0].swatches;
    assert_eq!(swatches.len(), 3);
    assert_eq!(swatches[0].swatch.y, swatches[1].swatch.y);
    assert!(swatches[2].swatch.y > swatches[0].swatch.y);
    assert_eq!(swatches[2].swatch.x, swatches[0].swatch.x);
    // A point's colour outlines a circle of Vega's 100 square pixels.
    let glyph = &swatches[0].glyph;
    assert_eq!(glyph.size, 100.0);
    assert!(glyph.fill.is_none() && glyph.stroke.is_some());
    // Its label stands 4 pixels past the symbol.
    let entry = &swatches[0];
    assert_eq!(entry.label.at[0], entry.swatch.right() + 4.0);
}

#[test]
fn a_continuous_colour_draws_a_gradient_and_channels_over_one_field_merge() {
    let plan = laid_out(&points().color(Channel::quantitative("y")));
    let legend = &plan.legends[0];
    let gradient = legend.gradient.as_ref().unwrap();
    assert!(gradient.vertical);
    assert_eq!(gradient.rect.w, 16.0);
    let Fill::Stops(stops) = &gradient.fill else {
        panic!()
    };
    assert_eq!(stops.first().unwrap().0, 0.0);
    assert_eq!(stops.last().unwrap().0, 1.0);
    assert!(!legend.labels.is_empty());

    // Colour and shape over one field: one legend of coloured shapes.
    let plan = laid_out(
        &points()
            .color(Channel::nominal("k"))
            .shape(Channel::nominal("k")),
    );
    assert_eq!(plan.legends.len(), 1);
    let glyphs: Vec<_> = plan.legends[0].swatches.iter().map(|s| &s.glyph).collect();
    assert_ne!(glyphs[0].shape, glyphs[1].shape);
    assert_ne!(glyphs[0].stroke, glyphs[1].stroke);

    // A stroke width legend, its lines as wide as the rows' are.
    let plan = laid_out(
        &Chart::line(data())
            .x(Channel::quantitative("x"))
            .y(Channel::quantitative("y"))
            .stroke_width(Channel::nominal("k")),
    );
    let widths: Vec<f32> = plan.legends[0]
        .swatches
        .iter()
        .map(|s| s.glyph.stroke_width)
        .collect();
    assert_eq!(widths, [1.0, 2.5, 4.0]);
    assert_eq!(
        plan.legends[0].swatches[0].glyph.shape,
        chart::Shape::Stroke
    );
}

#[test]
fn the_title_stands_over_everything_else() {
    let title = Title {
        subtitle: Some("sub".into()),
        anchor: Some(Anchor::Start),
        ..Title::new("One\nTwo")
    };
    let plan = laid_out(&points().color(Channel::nominal("k")).title(title));
    let (title, subtitle) = (
        plan.title.as_ref().unwrap(),
        plan.subtitle.as_ref().unwrap(),
    );
    assert_eq!(title.at, [plan.plot.x, 5.0]);
    assert_eq!(title.height(), 2.0 * title.line);
    assert_eq!(subtitle.at[1], title.at[1] + title.height() + 3.0);
    let others = plan
        .axes
        .iter()
        .filter_map(|axis| axis.bounds())
        .chain(plan.legends.iter().filter_map(|legend| legend.bounds()))
        .fold(plan.plot, Rect::union);
    assert_eq!(subtitle.at[1] + subtitle.height() + 4.0, others.y);

    let below = Title {
        orient: Some(Side::Bottom),
        ..Title::new("Under")
    };
    let plan = laid_out(&points().title(below));
    let title = plan.title.as_ref().unwrap();
    assert!(title.at[1] > plan.plot.bottom());
    assert_eq!(
        title.at[0] + title.width / 2.0,
        plan.plot.x + plan.plot.w / 2.0
    );

    // A title beside the chart stands level only where its angle says.
    let beside = Title {
        orient: Some(Side::Left),
        ..Title::new("Side")
    };
    assert!(!points().title(beside.clone()).drawable());
    let level = Title {
        lettering: Lettering {
            angle: Some(0.0),
            ..Lettering::default()
        },
        ..beside
    };
    let plan = laid_out(&points().title(level));
    assert!(plan.title.as_ref().unwrap().at[0] < plan.plot.x);
}

#[test]
fn text_at_an_angle_draws_nothing() {
    let angled = |axis: Axis| {
        !points()
            .x(Channel::quantitative("x").axis(Some(axis)))
            .drawable()
    };
    let labels = Axis {
        style: TickStyle {
            label: Lettering {
                angle: Some(-45.0),
                ..Lettering::default()
            },
            ..TickStyle::default()
        },
        ..Axis::default()
    };
    assert!(angled(labels));
    let level = Axis {
        title: Lettering {
            angle: Some(360.0),
            ..Lettering::default()
        },
        ..Axis::default()
    };
    assert!(!angled(level));
}
