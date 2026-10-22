//! Composite marks expand to Vega-Lite's part layers; their data is compared
//! with Vega's in `tests/transform.rs`.

use chart::{
    Chart, Encode, Mark,
    model::{Interpolate, Orient},
    vega_lite,
};
use gpui::{Hsla, Rgba};

/// `mark` over a nominal `g` and a quantitative `v`, and `encoding`'s other
/// channels.
fn import(mark: &str, encoding: &str) -> Option<Chart> {
    vega_lite::import(&format!(
        r#"{{
          "data": {{"values": [
            {{"g": "a", "v": 1}}, {{"g": "a", "v": 2}}, {{"g": "a", "v": 3}},
            {{"g": "a", "v": 40}}, {{"g": "b", "v": 5}}, {{"g": "b", "v": 6}}
          ]}},
          "mark": {mark},
          "encoding": {{{encoding}}}
        }}"#
    ))
}

const VERTICAL: &str =
    r#""x": {"field": "g", "type": "nominal"}, "y": {"field": "v", "type": "quantitative"}"#;
const HORIZONTAL: &str =
    r#""y": {"field": "g", "type": "nominal"}, "x": {"field": "v", "type": "quantitative"}"#;

fn marks(chart: &Chart) -> Vec<Mark> {
    chart.layers.iter().map(|layer| layer.mark).collect()
}

fn hex(hex: u32) -> Hsla {
    Rgba {
        a: 1.0,
        ..gpui::rgb(hex)
    }
    .into()
}

#[test]
fn a_boxplot_draws_outliers_whiskers_a_box_and_its_median() {
    let chart = import(r#""boxplot""#, VERTICAL).unwrap();
    assert_eq!(
        marks(&chart),
        [Mark::Point, Mark::Rule, Mark::Rule, Mark::Bar, Mark::Tick]
    );
    let bar = &chart.layers[3];
    assert_eq!(bar.properties.orient, Some(Orient::Vertical));
    assert_eq!(bar.encoding.size, Some(Encode::Value(14.0)));
    let median = &chart.layers[4];
    assert_eq!(median.properties.orient, Some(Orient::Horizontal));
    assert_eq!(median.encoding.color, Some(Encode::Value(hex(0xffffff))));

    let chart = import(r#""boxplot""#, HORIZONTAL).unwrap();
    assert_eq!(chart.layers[3].properties.orient, Some(Orient::Horizontal));
    assert!(chart.layers[3].encoding.x.is_some() && chart.layers[3].encoding.x2.is_some());
}

#[test]
fn a_boxplots_parts_are_drawn_as_its_properties_say() {
    let chart = import(
        r#"{"type": "boxplot", "extent": "min-max", "ticks": true, "outliers": true,
            "median": {"color": "red"}, "size": 20, "color": "teal", "opacity": 0.5}"#,
        VERTICAL,
    )
    .unwrap();
    // Min-max whiskers reach every value: no outliers.
    assert_eq!(
        marks(&chart),
        [
            Mark::Rule,
            Mark::Rule,
            Mark::Tick,
            Mark::Tick,
            Mark::Bar,
            Mark::Tick
        ]
    );
    let tick = &chart.layers[2];
    assert_eq!(tick.encoding.color, Some(Encode::Value(hex(0x000000))));
    assert_eq!(tick.encoding.opacity, Some(Encode::Value(1.0)));
    let bar = &chart.layers[4];
    assert_eq!(bar.encoding.color, Some(Encode::Value(hex(0x008080))));
    assert_eq!(bar.encoding.opacity, Some(Encode::Value(0.5)));
    assert_eq!(bar.encoding.size, Some(Encode::Value(20.0)));
    assert_eq!(
        chart.layers[5].encoding.color,
        Some(Encode::Value(hex(0xff0000)))
    );

    let chart = import(
        r#"{"type": "boxplot", "outliers": false, "median": false}"#,
        VERTICAL,
    )
    .unwrap();
    assert_eq!(marks(&chart), [Mark::Rule, Mark::Rule, Mark::Bar]);
}

#[test]
fn an_errorbar_is_a_rule_with_ticks_where_asked() {
    let chart = import(r#""errorbar""#, HORIZONTAL).unwrap();
    assert_eq!(marks(&chart), [Mark::Rule]);
    let chart = import(
        r#"{"type": "errorbar", "extent": "stdev", "ticks": true, "thickness": 3, "size": 9}"#,
        HORIZONTAL,
    )
    .unwrap();
    assert_eq!(marks(&chart), [Mark::Tick, Mark::Tick, Mark::Rule]);
    let tick = &chart.layers[0];
    assert_eq!(tick.properties.orient, Some(Orient::Vertical));
    assert_eq!(tick.properties.thickness, Some(3.0));
    assert_eq!(tick.encoding.size, Some(Encode::Value(9.0)));
    assert_eq!(chart.layers[2].encoding.size, Some(Encode::Value(3.0)));
}

#[test]
fn an_errorband_is_an_area_with_borders_where_asked() {
    let chart = import(r#""errorband""#, VERTICAL).unwrap();
    assert_eq!(marks(&chart), [Mark::Area]);
    assert_eq!(chart.layers[0].encoding.opacity, Some(Encode::Value(0.3)));
    let chart = import(
        r#"{"type": "errorband", "borders": true, "interpolate": "monotone", "tension": 0.5}"#,
        VERTICAL,
    )
    .unwrap();
    assert_eq!(marks(&chart), [Mark::Area, Mark::Line, Mark::Line]);
    for layer in &chart.layers {
        assert_eq!(layer.properties.interpolate, Interpolate::Monotone);
        assert_eq!(layer.properties.tension, Some(0.5));
    }
    let one_axis = import(
        r#"{"type": "errorband", "band": false, "borders": true}"#,
        r#""y": {"field": "v", "type": "quantitative"}"#,
    )
    .unwrap();
    assert_eq!(marks(&one_axis), [Mark::Rule, Mark::Rule]);
}

#[test]
fn a_composite_mark_vega_lite_cannot_orient_is_none() {
    let nominal =
        r#""x": {"field": "g", "type": "nominal"}, "y": {"field": "v", "type": "nominal"}"#;
    assert!(import(r#""boxplot""#, nominal).is_none());
    let both = r#""x": {"field": "v", "type": "quantitative", "aggregate": "boxplot"},
                 "y": {"field": "v", "type": "quantitative", "aggregate": "boxplot"}"#;
    assert!(import(r#""boxplot""#, both).is_none());
    let ends = format!(r#"{VERTICAL}, "x2": {{"field": "v"}}, "y2": {{"field": "v"}}"#);
    assert!(import(r#""errorbar""#, &ends).is_none());
    assert!(import(r#"{"type": "boxplot", "extent": "tukey"}"#, VERTICAL).is_none());
}
