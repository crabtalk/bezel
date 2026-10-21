use chart::{
    Column, Datum, Encode, Kind, Mark, Position, Sort, Source,
    model::{Align, Baseline, Cap, Dir, Extent, Interpolate, LineBreak},
    time, vega_lite,
};
use gpui::Rgba;

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
    assert_eq!(chart.layers[0].mark, Mark::Bar);
    assert_eq!(chart.title.as_deref(), Some("Sales"));
    assert_eq!(chart.layers[0].data.len(), 3);
    let Some(Column::Number(sales)) = chart.layers[0].data.column("sales") else {
        panic!()
    };
    assert_eq!(sales[0], 3.0);
    assert!(sales[1].is_nan());
    let Some(Column::Text(region)) = chart.layers[0].data.column("region") else {
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
    assert_eq!(
        chart.layers[0]
            .encoding
            .x
            .as_ref()
            .and_then(Encode::channel)
            .cloned()
            .unwrap()
            .kind,
        Kind::Temporal
    );
    let Some(Column::Number(days)) = chart.layers[0].data.column("day") else {
        panic!()
    };
    assert_eq!(days[1] - days[0], time::DAY);
}

#[test]
fn what_it_cannot_draw_as_written_is_none() {
    let with = |change: &str| BARS.replacen("\"mark\"", &format!("{change}, \"mark\""), 1);
    assert!(vega_lite::import(&with(r#""transform": [{"filter": "datum.sales > 1"}]"#)).is_none());
    assert!(
        vega_lite::import(&BARS.replace(
            r#""type": "quantitative""#,
            r#""type": "quantitative", "aggregate": "sum""#
        ))
        .is_none()
    );
    assert!(vega_lite::import(&BARS.replace("\"tooltip\": [", "\"size\": [")).is_none());
    assert!(vega_lite::import(&BARS.replace("\"bar\"", "\"geoshape\"")).is_none());
    assert!(vega_lite::import(&BARS.replace("\"y\": {", "\"y2\": {")).is_none());
    assert!(
        vega_lite::import(r#"{"data": {"url": "x.csv"}, "mark": "bar", "encoding": {}}"#).is_none()
    );
    assert!(vega_lite::import("not json").is_none());
}

#[test]
fn channels_take_vega_lites_defaults_and_settings() {
    let chart = vega_lite::import(BARS).unwrap();
    let x = chart.layers[0]
        .encoding
        .x
        .as_ref()
        .and_then(Encode::channel)
        .cloned()
        .unwrap();
    assert_eq!(x.title.as_deref(), Some("month"));
    assert_eq!(x.sort, Sort::Ascending);

    let spec = BARS
        .replace(
            r#""y": {"field": "sales", "type": "quantitative"}"#,
            r#""y": {"field": "sales", "type": "quantitative", "title": null, "scale": {"zero": false, "domain": [1, 9]}}"#,
        )
        .replace(r#""axis": {"labelAngle": 0}"#, r#""axis": {"title": "Month"}, "sort": ["Mar"]"#);
    let chart = vega_lite::import(&spec).unwrap();
    let (x, y) = (
        chart.layers[0]
            .encoding
            .x
            .as_ref()
            .and_then(Encode::channel)
            .cloned()
            .unwrap(),
        chart.layers[0]
            .encoding
            .y
            .as_ref()
            .and_then(Encode::channel)
            .cloned()
            .unwrap(),
    );
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

#[test]
fn a_layer_takes_the_top_levels_data_and_encoding() {
    let spec = r#"{
      "data": {"values": [{"m": "a", "v": 1, "w": 2}, {"m": "b", "v": 3, "w": 1}]},
      "encoding": {"x": {"field": "m", "type": "ordinal"}},
      "layer": [
        {"mark": "bar", "encoding": {"y": {"field": "v", "type": "quantitative"}}},
        {"mark": "line", "encoding": {"y": {"field": "w", "type": "quantitative"}}}
      ]
    }"#;
    let chart = vega_lite::import(spec).unwrap();
    assert_eq!(chart.layers.len(), 2);
    let line = &chart.layers[1];
    assert_eq!(line.mark, Mark::Line);
    assert_eq!(
        line.encoding
            .x
            .as_ref()
            .and_then(Encode::channel)
            .unwrap()
            .field()
            .unwrap()
            .as_ref(),
        "m"
    );
    assert_eq!(
        line.encoding
            .y
            .as_ref()
            .and_then(Encode::channel)
            .unwrap()
            .field()
            .unwrap()
            .as_ref(),
        "w"
    );
    assert_eq!(line.data.len(), 2);

    let nested = spec.replace(r#"{"mark": "line""#, r#"{"layer": [], "mark": "line""#);
    assert!(vega_lite::import(&nested).is_none());
}

#[test]
fn every_channel_reads_a_field_a_datum_or_a_value() {
    let spec = r##"{
      "data": {"values": [
        {"day": "2024-01-02", "end": "2024-01-05", "task": "a", "n": 1, "url": "https://a"},
        {"day": "2024-01-03", "end": "2024-01-04", "task": "b", "n": 2, "url": "https://b"}
      ]},
      "mark": "bar",
      "encoding": {
        "y": {"field": "task"},
        "x": {"field": "day", "type": "temporal"},
        "x2": {"field": "end"},
        "color": {"value": "steelblue"},
        "stroke": {"value": null},
        "opacity": {"datum": 0.5},
        "strokeDash": {"value": [4, 2]},
        "shape": {"field": "task"},
        "href": {"field": "url"},
        "detail": [{"field": "task"}, {"field": "n", "type": "quantitative"}],
        "order": {"field": "n", "sort": "descending"},
        "key": {"field": "task"}
      }
    }"##;
    let chart = vega_lite::import(spec).unwrap();
    let e = &chart.layers[0].encoding;
    let y = e.y.as_ref().and_then(Encode::channel).unwrap();
    assert_eq!(y.kind, Kind::Nominal);
    let x2 = e.x2.as_ref().and_then(Encode::channel).unwrap();
    assert_eq!(x2.kind, Kind::Temporal);
    let Some(Column::Number(end)) = chart.layers[0].data.column("end") else {
        panic!("x2 reads its field as times")
    };
    assert_eq!(end[0], time::parse("2024-01-05").unwrap());
    let Some(Encode::Value(color)) = e.color else {
        panic!()
    };
    assert!((Rgba::from(color).b - 180.0 / 255.0).abs() < 1e-4);
    assert!(matches!(e.stroke, Some(Encode::Value(none)) if none.a == 0.0));
    let opacity = e.opacity.as_ref().and_then(Encode::channel).unwrap();
    assert_eq!(opacity.source, Source::Datum(Datum::Number(0.5)));
    assert_eq!(opacity.kind, Kind::Quantitative);
    assert_eq!(e.stroke_dash, Some(Encode::Value(vec![4.0, 2.0])));
    assert_eq!(e.detail.len(), 2);
    assert_eq!(e.order[0].sort, Sort::Descending);
    assert_eq!(e.order[0].kind, Kind::Ordinal);
    assert!(e.key.is_some() && e.href.is_some());
}

const POINTS: &str = r#"{"data": {"values": [{"a": 1, "b": 2}, {"a": 2, "b": 3}]}, "mark": "point", "encoding": {"x": {"field": "a", "type": "quantitative"}, "y": {"field": "b", "type": "quantitative"}}}"#;

#[test]
fn a_mark_definition_reads_its_properties() {
    let mark = |def: &str| {
        let chart = vega_lite::import(&POINTS.replace(r#""point""#, def));
        chart.map(|chart| chart.layers[0].clone())
    };
    let square = mark(r#""square""#).unwrap();
    assert_eq!(square.mark, Mark::Square);
    assert_eq!(square.encoding.shape, None);

    let line = mark(
        r#"{"type": "line", "interpolate": "monotone", "strokeCap": "round", "strokeDashOffset": 2, "point": {"filled": false, "fill": "white"}}"#,
    )
    .unwrap();
    let p = &line.properties;
    assert_eq!(p.interpolate, Interpolate::Monotone);
    assert_eq!(p.stroke_cap, Cap::Round);
    assert_eq!(p.stroke_dash_offset, 2.0);
    let point = p.point.as_ref().unwrap();
    assert_eq!(point.properties.filled, Some(false));
    assert!(matches!(point.encoding.fill, Some(Encode::Value(white)) if white.l == 1.0));
    assert_eq!(point.encoding.opacity, Some(Encode::Value(1.0)));
    assert!(
        mark(r#"{"type": "line", "point": "transparent"}"#)
            .unwrap()
            .properties
            .point
            .is_none()
    );

    let bar = mark(r#"{"type": "rule", "color": "red", "size": 3, "opacity": 0.4}"#).unwrap();
    assert!(matches!(bar.encoding.color, Some(Encode::Value(_))));
    assert_eq!(bar.encoding.size, Some(Encode::Value(3.0)));
    assert_eq!(bar.encoding.opacity, Some(Encode::Value(0.4)));

    let text = mark(
        r#"{"type": "text", "text": "hi", "align": "left", "baseline": "top", "dx": 3, "fontWeight": "bold", "fontSize": 14, "limit": 40}"#,
    )
    .unwrap();
    let p = &text.properties;
    assert_eq!(
        (p.align, p.baseline),
        (Some(Align::Left), Some(Baseline::Top))
    );
    assert_eq!((p.dx, p.font_size, p.limit), (3.0, Some(14.0), 40.0));
    assert_eq!(text.encoding.text, Some(Encode::Value("hi".into())));

    let tick = mark(r#"{"type": "tick", "thickness": 2, "width": {"band": 0.5}, "cornerRadiusTopLeft": 1, "cornerRadius": 3}"#).unwrap();
    assert_eq!(tick.properties.thickness, Some(2.0));
    assert_eq!(tick.properties.width, Some(Extent::Band(0.5)));
    assert_eq!(
        tick.properties.corner_radius,
        [Some(1.0), Some(3.0), Some(3.0), Some(3.0)]
    );

    // An encoded colour paints over a fill property.
    let colored = POINTS.replace(
        r#""encoding": {"#,
        r#""encoding": {"color": {"field": "a", "type": "nominal"}, "#,
    );
    let circle =
        vega_lite::import(&colored.replace(r#""point""#, r#"{"type": "circle", "fill": "red"}"#))
            .unwrap();
    assert_eq!(circle.layers[0].encoding.fill, None);

    assert!(
        mark(r#"{"type": "point", "clip": true}"#)
            .unwrap()
            .properties
            .clip
    );
    assert!(!square.properties.clip);
    assert!(mark(r#"{"type": "point", "clip": {"expr": "true"}}"#).is_none());
    assert!(mark(r#"{"type": "point", "smooth": true}"#).is_none());
    assert!(mark(r#"{"type": "geoshape"}"#).is_none());
    assert!(mark(r#"{"type": "text", "angle": 45}"#).is_none());
}

#[test]
fn a_channel_it_cannot_read_is_none() {
    let with = |channel: &str| {
        format!(
            r#"{{"data": {{"values": [{{"a": 1}}]}}, "mark": "point", "encoding": {{"x": {{"field": "a", "type": "quantitative"}}, "y": {{"field": "a", "type": "quantitative"}}, {channel}}}}}"#
        )
    };
    assert!(vega_lite::import(&with(r#""shape": {"value": "M0,0L1,1Z"}"#)).is_none());
    assert!(vega_lite::import(&with(r#""color": {"value": "nope"}"#)).is_none());
    assert!(vega_lite::import(&with(r#""order": {"value": 0}"#)).is_none());
    assert!(
        vega_lite::import(&with(
            r#""opacity": {"condition": {"test": "true", "value": 1}, "value": 0.2}"#
        ))
        .is_none()
    );
    assert!(vega_lite::import(&with(r#""y2": {"field": "a"}"#)).is_none());
    assert!(vega_lite::import(&with(r#""x2": {"field": "a", "type": "quantitative"}"#)).is_none());
}

#[test]
fn a_datetime_names_a_time_on_the_charts_calendar() {
    let at = |datum: &str| {
        let spec = format!(
            r#"{{"data": {{"values": [{{"a": 1}}]}}, "layer": [
              {{"mark": "point", "encoding": {{"x": {{"field": "a", "type": "quantitative"}}, "y": {{"field": "a", "type": "quantitative"}}}}}},
              {{"mark": "rule", "encoding": {{"y": {{"datum": {datum}}}}}}}
            ]}}"#
        );
        let chart = vega_lite::import(&spec)?;
        let y = chart.layers[1].encoding.y.clone()?;
        let channel = y.channel()?.clone();
        assert_eq!(channel.kind, Kind::Temporal);
        match channel.source {
            Source::Datum(Datum::Number(time)) => Some(time),
            _ => None,
        }
    };
    let parse = |text: &str| time::parse(text).unwrap();
    assert_eq!(at(r#"{"year": 2006}"#), Some(parse("2006-01-01T00:00Z")));
    assert_eq!(
        at(r#"{"year": 2006, "month": "Mar", "date": 5, "hours": 13, "utc": true}"#),
        Some(parse("2006-03-05T13:00Z"))
    );
    assert_eq!(
        at(r#"{"year": 2006, "quarter": 3}"#),
        Some(parse("2006-07-01"))
    );
    assert_eq!(at(r#"{"month": "september"}"#), Some(parse("2012-09-01")));
    // A day alone is that weekday in the first week of 2012, which begins on
    // a Sunday; beside another field it is dropped.
    assert_eq!(at(r#"{"day": "tue"}"#), Some(parse("2012-01-03")));
    assert_eq!(at(r#"{"day": 2, "year": 2001}"#), Some(parse("2001-01-01")));
    assert_eq!(at(r#"{"week": 3}"#), None);
    assert_eq!(at(r#"{"month": "smarch"}"#), None);

    let domain = r#"{"data": {"values": [{"t": "2001-05-01"}]}, "mark": "point", "encoding": {"x": {"field": "t", "type": "temporal", "scale": {"domain": [{"year": 2001}, {"year": 2002}]}}, "y": {"field": "t", "type": "temporal"}}}"#;
    let chart = vega_lite::import(domain).unwrap();
    let x = chart.layers[0]
        .encoding
        .x
        .as_ref()
        .and_then(Encode::channel)
        .unwrap();
    assert_eq!(
        x.scale.domain,
        Some([parse("2001-01-01"), parse("2002-01-01")])
    );
}

#[test]
fn positions_offsets_and_text_properties_are_read() {
    let mark = |def: &str| {
        let chart = vega_lite::import(&POINTS.replace(r#""point""#, def));
        chart.map(|chart| chart.layers[0].clone())
    };
    let rule =
        mark(r#"{"type": "rule", "x2": "width", "y2": 4, "x2Offset": 2, "y2Offset": -1}"#).unwrap();
    assert_eq!(rule.encoding.x2, Some(Encode::Value(Position::Size)));
    assert_eq!(rule.encoding.y2, Some(Encode::Value(Position::Pixels(4.0))));
    assert_eq!(
        (rule.properties.x2_offset, rule.properties.y2_offset),
        (2.0, -1.0)
    );
    assert!(mark(r#"{"type": "rule", "x2": "height"}"#).is_none());

    let text =
        mark(r#"{"type": "text", "text": "a b", "ellipsis": "~", "dir": "rtl", "lineBreak": " "}"#)
            .unwrap();
    let p = &text.properties;
    assert_eq!(p.ellipsis.as_deref(), Some("~"));
    assert_eq!(p.dir, Dir::Rtl);
    assert_eq!(p.line_break, Some(LineBreak::Text(" ".into())));
    let pattern =
        mark(r#"{"type": "text", "text": "a", "lineBreak": {"expr": "/[,;]/g"}}"#).unwrap();
    assert_eq!(
        pattern.properties.line_break,
        Some(LineBreak::Pattern {
            source: "[,;]".into(),
            flags: "g".into()
        })
    );
    assert!(mark(r#"{"type": "text", "text": "a", "lineBreak": {"expr": "datum.a"}}"#).is_none());
    assert!(mark(r#"{"type": "text", "text": "a", "stroke": "white"}"#).is_none());

    let arc = |def: &str| {
        let spec = format!(
            r#"{{"data": {{"values": [{{"a": 1}}, {{"a": 2}}]}}, "mark": {def}, "encoding": {{"theta": {{"field": "a", "type": "quantitative"}}}}}}"#
        );
        vega_lite::import(&spec).map(|chart| chart.layers[0].properties.clone())
    };
    let p = arc(r#"{"type": "arc", "cornerRadius": 6, "cornerRadiusTopLeft": 1, "thetaOffset": 0.2, "theta2Offset": 0.1, "radiusOffset": 3, "radius2Offset": 2}"#).unwrap();
    assert_eq!(p.corner_radius, [Some(6.0); 4]);
    assert_eq!(
        (
            p.theta_offset,
            p.theta2_offset,
            p.radius_offset,
            p.radius2_offset
        ),
        (0.2, 0.1, 3.0, 2.0)
    );
}
