use chart::{Column, Kind, Mark, Sort, time, vega_lite};

const BARS: &str = r#"{
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "title": {"text": "Sales"},
  "data": {"values": [
    {"month": "Jan", "sales": 3, "region": "east"},
    {"month": "Feb", "sales": null, "region": "west"},
    {"month": "Mar", "sales": 4}
  ]},
  "mark": {"type": "bar", "tooltip": true},
  "encoding": {
    "x": {"field": "month", "type": "nominal", "axis": {"labelAngle": 0}},
    "y": {"field": "sales", "type": "quantitative"},
    "color": {"field": "region", "type": "nominal"},
    "tooltip": [{"field": "sales"}]
  }
}"#;

#[test]
fn a_spec_becomes_columns_named_by_its_encoding() {
    let chart = vega_lite::import(BARS).unwrap();
    assert_eq!(chart.mark, Mark::Bar);
    assert_eq!(chart.title.as_deref(), Some("Sales"));
    assert_eq!(chart.data.len(), 3);
    let Some(Column::Number(sales)) = chart.data.column("sales") else {
        panic!()
    };
    assert_eq!(sales[0], 3.0);
    assert!(sales[1].is_nan());
    let Some(Column::Text(region)) = chart.data.column("region") else {
        panic!()
    };
    assert_eq!(region.get(2), "");
}

#[test]
fn temporal_strings_are_read_as_times() {
    let spec = r#"{
      "data": {"values": [{"day": "2024-01-02", "v": 1}, {"day": "2024-01-03", "v": 2}]},
      "mark": "line",
      "encoding": {"x": {"field": "day", "type": "temporal"}, "y": {"field": "v", "type": "quantitative"}}
    }"#;
    let chart = vega_lite::import(spec).unwrap();
    assert_eq!(chart.encoding.x.unwrap().kind, Kind::Temporal);
    let Some(Column::Number(days)) = chart.data.column("day") else {
        panic!()
    };
    assert_eq!(days[1] - days[0], time::DAY);
}

#[test]
fn what_it_cannot_draw_as_written_is_none() {
    let with = |change: &str| BARS.replacen("\"mark\"", &format!("{change}, \"mark\""), 1);
    assert!(vega_lite::import(&with(r#""transform": []"#)).is_none());
    assert!(
        vega_lite::import(&BARS.replace(
            r#""type": "quantitative""#,
            r#""type": "quantitative", "aggregate": "sum""#
        ))
        .is_none()
    );
    assert!(vega_lite::import(&BARS.replace("\"tooltip\": [", "\"size\": [")).is_none());
    assert!(vega_lite::import(&BARS.replace("\"bar\"", "\"rule\"")).is_none());
    assert!(
        vega_lite::import(r#"{"data": {"url": "x.csv"}, "mark": "bar", "encoding": {}}"#).is_none()
    );
    assert!(vega_lite::import("not json").is_none());
}

#[test]
fn channels_take_vega_lites_defaults_and_settings() {
    let chart = vega_lite::import(BARS).unwrap();
    let x = chart.encoding.x.unwrap();
    assert_eq!(x.title.as_deref(), Some("month"));
    assert_eq!(x.sort, Sort::Ascending);

    let spec = BARS
        .replace(
            r#""y": {"field": "sales", "type": "quantitative"}"#,
            r#""y": {"field": "sales", "type": "quantitative", "title": null, "scale": {"zero": false, "domain": [1, 9]}}"#,
        )
        .replace(r#""axis": {"labelAngle": 0}"#, r#""axis": {"title": "Month"}, "sort": ["Mar"]"#);
    let chart = vega_lite::import(&spec).unwrap();
    let (x, y) = (chart.encoding.x.unwrap(), chart.encoding.y.unwrap());
    assert_eq!(x.title.as_deref(), Some("Month"));
    assert_eq!(x.sort, Sort::Explicit(vec!["Mar".into()]));
    assert_eq!(y.title, None);
    assert!(!y.scale.zero);
    assert_eq!(y.scale.domain, Some([1.0, 9.0]));
}

#[test]
fn a_scale_or_sort_it_cannot_honour_is_none() {
    let y = r#""y": {"field": "sales", "type": "quantitative"}"#;
    let with = |channel: &str| BARS.replace(y, channel);
    assert!(
        vega_lite::import(&with(
            r#""y": {"field": "sales", "type": "quantitative", "scale": {"type": "log"}}"#
        ))
        .is_none()
    );
    assert!(
        vega_lite::import(&with(
            r#""y": {"field": "sales", "type": "quantitative", "stack": "normalize"}"#
        ))
        .is_none()
    );
    assert!(
        vega_lite::import(&BARS.replace(
            r#""type": "nominal", "axis""#,
            r#""type": "nominal", "sort": "-y", "axis""#
        ))
        .is_none()
    );
}
