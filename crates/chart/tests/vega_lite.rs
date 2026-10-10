use chart::{Column, Kind, Mark, time, vega_lite};

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
