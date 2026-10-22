//! An `axis`, a `legend` and the top-level `title`, read by every property.

use chart::{
    Channel, Chart, Datum, Encode,
    model::{
        Anchor, Format, Frame, GridAlign, LegendKind, LegendOrient, Orient, Overlap, Side,
        TickBand, TickCount,
    },
    plan::Metrics,
    vega_lite,
};
use gpui::{FontWeight, Rgba};
use js::time::Unit;

fn spec(x: &str, color: &str, title: &str) -> String {
    format!(
        r#"{{
      "title": {title},
      "data": {{"values": [{{"a": 1, "b": 2, "c": "p"}}, {{"a": 2, "b": 0, "c": "q"}}]}},
      "mark": "point",
      "encoding": {{
        "x": {x},
        "y": {{"field": "b", "type": "quantitative"}},
        "color": {color}
      }}
    }}"#
    )
}

fn channel(chart: &Chart, pick: fn(&chart::Encoding) -> Option<&Channel>) -> Channel {
    pick(&chart.layers[0].encoding)
        .cloned()
        .expect("the channel")
}

fn x(chart: &Chart) -> Channel {
    channel(chart, |e| e.x.as_ref().and_then(Encode::channel))
}

fn color(chart: &Chart) -> Channel {
    channel(chart, |e| e.color.as_ref().and_then(Encode::channel))
}

const COLOR: &str = r#"{"field": "c", "type": "nominal"}"#;
const TITLE: &str = r#""Plain""#;

#[test]
fn an_axis_reads_every_property() {
    let chart = vega_lite::import(&spec(
        r#"{"field": "a", "type": "quantitative", "axis": {
          "orient": "top", "offset": 3, "position": 2, "translate": 1, "minExtent": 10,
          "maxExtent": 90, "bandPosition": 0, "zindex": 1, "domain": false, "domainColor": "red",
          "domainWidth": 2, "domainDash": [2, 1], "domainCap": "round", "domainOpacity": 0.5,
          "domainDashOffset": 1, "grid": true, "gridColor": "blue", "gridWidth": 3,
          "ticks": true, "tickColor": "green", "tickSize": 8, "tickWidth": 2, "tickOffset": 1,
          "tickRound": false, "tickBand": "extent", "tickExtra": true, "tickCount": 4,
          "tickMinStep": 0.5, "values": [1, 2], "labels": true, "format": ".2f",
          "labelExpr": "datum.label + '!'", "labelColor": "red", "labelFont": "Mono",
          "labelFontSize": 9, "labelFontWeight": "bold", "labelFontStyle": "italic",
          "labelLimit": 50, "labelLineHeight": 11, "labelOpacity": 0.8, "labelAlign": "left",
          "labelBaseline": "top", "labelAngle": 0, "labelBound": true, "labelFlush": 2,
          "labelFlushOffset": 1, "labelOffset": 3, "labelPadding": 4, "labelOverlap": "greedy",
          "labelSeparation": 2, "title": "A", "titleAnchor": "end", "titlePadding": 6,
          "titleX": 1, "titleY": 2, "titleColor": "red", "titleFontSize": 12, "titleAngle": 0,
          "titleAlign": "right", "titleBaseline": "bottom", "titleLimit": 80,
          "aria": false, "description": "x", "style": "s"
        }}"#,
        COLOR,
        TITLE,
    ))
    .unwrap();
    let x = x(&chart);
    assert_eq!(x.title.as_deref(), Some("A"));
    let axis = x.axis.as_deref().unwrap();
    assert_eq!(axis.orient, Some(Side::Top));
    assert_eq!(
        (axis.offset, axis.position, axis.translate),
        (Some(3.0), Some(2.0), Some(1.0))
    );
    assert_eq!((axis.min_extent, axis.max_extent), (Some(10.0), Some(90.0)));
    assert_eq!(axis.domain.show, Some(false));
    assert_eq!(axis.domain.dash, Some(vec![2.0, 1.0]));
    assert_eq!(axis.style.grid.width, Some(3.0));
    assert_eq!(axis.style.size, Some(8.0));
    assert_eq!(axis.tick_band, Some(TickBand::Extent));
    assert_eq!(axis.tick_count, Some(TickCount::Count(4.0)));
    assert_eq!(
        axis.values,
        Some(vec![Datum::Number(1.0), Datum::Number(2.0)])
    );
    assert_eq!(axis.format, Some(Format::Number(".2f".parse().unwrap())));
    assert_eq!(axis.style.label.weight, Some(FontWeight::BOLD));
    assert_eq!(axis.label_flush, Some(Some(2.0)));
    assert_eq!(axis.label_bound, Some(1.0));
    assert_eq!(axis.label_overlap, Some(Overlap::Greedy));
    assert_eq!(axis.title_anchor, Some(Anchor::End));
    assert_eq!(axis.title.angle, Some(0.0));
    let plan = chart::plan(
        &chart,
        [400.0, 300.0],
        Metrics {
            size: 10.0,
            line: 10.0,
        },
        &mut |_, _| 10.0,
    )
    .unwrap();
    let top = plan.axes.iter().find(|a| a.side == Side::Top).unwrap();
    assert!(top.labels.iter().all(|label| label.text.ends_with('!')));
    assert!(top.over);

    // A property this does not read, or an expression for a value, is none.
    let none = |axis: &str| {
        let x = format!(r#"{{"field": "a", "type": "quantitative", "axis": {axis}}}"#);
        vega_lite::import(&spec(&x, COLOR, TITLE)).is_none()
    };
    assert!(none(r#"{"encode": {}}"#));
    assert!(none(r#"{"tickSize": {"expr": "2"}}"#));
    assert!(none(r#"{"labelAngle": 45}"#));
    assert!(none(r#"{"titleAngle": 270}"#));
    assert!(!none(r#"null"#));
}

#[test]
fn an_axis_property_takes_a_condition_per_tick() {
    let chart = vega_lite::import(&spec(
        r##"{"field": "a", "type": "quantitative", "axis": {
          "gridColor": {"condition": {"test": "datum.value === 1", "value": "black"}, "value": "#ddd"},
          "tickSize": {"condition": [{"test": "datum.value > 1", "value": 9}], "value": 3}
        }}"##,
        COLOR,
        TITLE,
    ))
    .unwrap();
    let axis = x(&chart).axis.unwrap();
    assert_eq!(axis.cases.len(), 2);
    assert_eq!(axis.style.size, Some(3.0));
    let one = &axis.cases[0];
    assert!(one.test.0(&Datum::Number(1.0), "1"));
    assert!(!one.test.0(&Datum::Number(2.0), "2"));
    assert_eq!(axis.cases[1].style.size, Some(9.0));
}

#[test]
fn a_time_units_axis_writes_the_unit() {
    let format = |unit: &str| {
        let x = format!(r#"{{"field": "t", "timeUnit": "{unit}", "type": "ordinal"}}"#);
        let chart = vega_lite::import(
            &spec(&x, COLOR, TITLE).replace(r#""a": 1"#, r#""t": "2024-03-05", "a": 1"#),
        )
        .unwrap();
        x_axis(&chart)
    };
    fn x_axis(chart: &Chart) -> (Option<Format>, Option<TickCount>, Option<f64>) {
        let axis = x(chart).axis.unwrap();
        (axis.format, axis.tick_count, axis.tick_min_step)
    }
    let day = 24.0 * 3600e3;
    assert_eq!(
        format("month"),
        (
            Some(Format::Time("%b".into())),
            Some(TickCount::Count(10.0)),
            Some(31.0 * day)
        )
    );
    assert_eq!(
        format("yearmonthdate").0,
        Some(Format::Time("%b %d, %Y".into()))
    );
    assert_eq!(format("hoursminutes").0, Some(Format::Time("%H:%M".into())));
    assert_eq!(
        format("day"),
        (
            Some(Format::Time("%a".into())),
            Some(TickCount::Count(10.0)),
            None
        )
    );
    let interval = vega_lite::import(&spec(
        r#"{"field": "a", "type": "temporal", "axis": {"tickCount": {"interval": "month", "step": 3}}}"#,
        COLOR,
        TITLE,
    ))
    .unwrap();
    assert_eq!(
        x(&interval).axis.unwrap().tick_count,
        Some(TickCount::Every(Unit::Month, 3.0))
    );
}

#[test]
fn a_legend_reads_every_property() {
    let chart = vega_lite::import(&spec(
        r#"{"field": "a", "type": "quantitative"}"#,
        r#"{"field": "c", "type": "nominal", "legend": {
          "type": "symbol", "orient": "bottom-left", "legendX": 1, "legendY": 2,
          "direction": "horizontal", "offset": 4, "padding": 5, "cornerRadius": 3,
          "fillColor": "white", "strokeColor": "gray", "columns": 2, "columnPadding": 6,
          "rowPadding": 7, "gridAlign": "all", "clipHeight": 12, "tickCount": 3,
          "tickMinStep": 1, "values": ["p"], "format": "d", "formatType": "number",
          "labelExpr": "upper(datum.label)", "labelOffset": 8, "labelOverlap": true,
          "labelSeparation": 1, "labelPadding": 2, "labelColor": "red", "labelLimit": 40,
          "title": "C", "titleAnchor": "middle", "titleOrient": "left", "titlePadding": 9,
          "titleFontSize": 13, "symbolType": "diamond", "symbolSize": 50,
          "symbolStrokeWidth": 2, "symbolFillColor": "red", "symbolStrokeColor": "blue",
          "symbolOpacity": 0.5, "symbolDash": [2, 2], "symbolDashOffset": 1,
          "symbolOffset": 3, "symbolLimit": 10, "gradientLength": 120,
          "gradientThickness": 10, "gradientStrokeColor": "black", "gradientStrokeWidth": 1,
          "gradientOpacity": 0.9, "zindex": 0, "aria": true
        }}"#,
        TITLE,
    ))
    .unwrap();
    let colored = color(&chart);
    assert_eq!(colored.title.as_deref(), Some("C"));
    let legend = colored.legend.as_deref().unwrap();
    assert_eq!(legend.kind, Some(LegendKind::Symbol));
    assert_eq!(legend.orient, Some(LegendOrient::BottomLeft));
    assert_eq!(legend.direction, Some(Orient::Horizontal));
    assert_eq!(legend.columns, Some(2));
    assert_eq!(legend.grid_align, Some(GridAlign::All));
    assert_eq!(legend.title_orient, Some(Side::Left));
    assert_eq!(legend.symbols.shape, Some(chart::Shape::Diamond));
    assert_eq!(legend.symbols.limit, Some(10));
    assert_eq!(legend.ramp.length, Some(120.0));
    assert_eq!(legend.values, Some(vec![Datum::Text("p".into())]));
    let plan = chart::plan(
        &chart,
        [400.0, 300.0],
        Metrics {
            size: 10.0,
            line: 10.0,
        },
        &mut |_, _| 10.0,
    )
    .unwrap();
    let swatches = &plan.legends[0].swatches;
    assert_eq!(swatches.len(), 1);
    assert_eq!(swatches[0].label.text, "P");
    assert!(plan.legends[0].frame.is_some());

    let hidden = vega_lite::import(&spec(
        r#"{"field": "a", "type": "quantitative"}"#,
        r#"{"field": "c", "type": "nominal", "legend": null}"#,
        TITLE,
    ))
    .unwrap();
    assert!(color(&hidden).legend.is_none());
}

#[test]
fn the_title_reads_every_property() {
    let chart = vega_lite::import(&spec(
        r#"{"field": "a", "type": "quantitative"}"#,
        COLOR,
        r#"{"text": ["One", "Two"], "subtitle": "Sub", "anchor": "start", "orient": "bottom",
          "offset": 7, "frame": "bounds", "align": "left", "baseline": "top", "angle": 0,
          "color": "red", "dx": 1, "dy": 2, "font": "Serif", "fontSize": 20,
          "fontStyle": "italic", "fontWeight": 300, "limit": 200, "lineHeight": 22,
          "subtitleColor": "gray", "subtitleFont": "Sans", "subtitleFontSize": 10,
          "subtitleFontStyle": "normal", "subtitleFontWeight": "bold",
          "subtitleLineHeight": 12, "subtitlePadding": 4, "zindex": 1, "aria": false,
          "style": "group-title"}"#,
    ))
    .unwrap();
    let title = chart.title.unwrap();
    assert_eq!(title.text, "One\nTwo");
    assert_eq!(title.subtitle.as_deref(), Some("Sub"));
    assert_eq!(title.anchor, Some(Anchor::Start));
    assert_eq!(title.orient, Some(Side::Bottom));
    assert_eq!(title.frame, Some(Frame::Bounds));
    assert_eq!((title.dx, title.dy), (1.0, 2.0));
    assert_eq!(title.lettering.size, Some(20.0));
    assert_eq!(title.lettering.weight, Some(FontWeight(300.0)));
    assert_eq!(
        title.lettering.color.map(Rgba::from),
        Some(Rgba {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0
        })
    );
    assert_eq!(title.subtitle_lettering.size, Some(10.0));
    assert_eq!(title.subtitle_padding, Some(4.0));

    let none = |title: &str| {
        vega_lite::import(&spec(
            r#"{"field": "a", "type": "quantitative"}"#,
            COLOR,
            title,
        ))
        .is_none()
    };
    assert!(none(r#"{"text": "T", "angle": 10}"#));
    assert!(none(r#"{"text": "T", "orient": "left"}"#));
    assert!(!none(r#"{"text": "T", "orient": "left", "angle": 0}"#));
    assert!(none(r#"{"text": "T", "encode": {}}"#));
}
