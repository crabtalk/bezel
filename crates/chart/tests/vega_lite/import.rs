use chart::{
    Channel, Chart, Column, Datum, Encode, Kind, Mark, Plan, Position, Shape, Sort, Source,
    model::{
        Align, Baseline, Cap, Dir, Domain, Extent, Interpolate, Invalid, LineBreak, NamedRange,
        Nice, Palette, Range, ScaleType, Side, Stack,
    },
    plan::Metrics,
    time,
    transform::Bins,
    vega_lite,
};
use gpui::Rgba;
use js::interpolate;

/// `chart` at 400 by 300, its text 10 pixels and every label 10 wide.
fn laid_out(chart: &Chart) -> Plan {
    let metrics = Metrics {
        size: 10.0,
        line: 10.0,
    };
    chart::plan(chart, [400.0, 300.0], metrics, &mut |_, _| 10.0).unwrap()
}

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
    assert_eq!(
        chart.title.as_ref().map(|title| title.text.as_ref()),
        Some("Sales")
    );
    assert_eq!(chart.layers[0].data.len(), 3);
    let Some(Column::Number(sales)) = chart.layers[0].data.column("sales") else {
        panic!()
    };
    assert_eq!(sales[0], 3.0);
    assert!(sales[1].is_nan());
    let Some(Column::Text(region)) = chart.layers[0].data.column("region") else {
        panic!()
    };
    assert_eq!(region.get(2), None);
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
            r#""type": "quantitative", "aggregate": "values""#
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
    assert_eq!(y.scale.zero, Some(false));
    assert_eq!(y.scale.domain, Some(Domain::Numbers(vec![1.0, 9.0])));
}

#[test]
fn a_scale_or_sort_it_cannot_honour_is_none() {
    let y = r#""y": {"field": "sales", "type": "quantitative"}"#;
    let with = |channel: &str| BARS.replace(y, channel);
    assert!(
        vega_lite::import(&with(
            r#""y": {"field": "sales", "type": "quantitative", "scale": {"range": {"step": 20}}}"#
        ))
        .is_none()
    );
    assert!(
        vega_lite::import(&with(
            r#""y": {"field": "sales", "type": "quantitative", "scale": {"scheme": "nonesuch"}}"#
        ))
        .is_none()
    );
    assert!(
        vega_lite::import(&with(
            r#""y": {"field": "sales", "type": "quantitative", "stack": "sideways"}"#
        ))
        .is_none()
    );
    assert!(
        vega_lite::import(&BARS.replace(
            r#""type": "nominal", "axis""#,
            r#""type": "nominal", "sort": "-size", "axis""#
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
    assert!(
        vega_lite::import(&with(
            r#""shape": {"value": "neither a symbol nor a path"}"#
        ))
        .is_none()
    );
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
        Some(Domain::Numbers(vec![
            parse("2001-01-01"),
            parse("2002-01-01")
        ]))
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

#[test]
fn a_scale_reads_every_property_and_a_stack_its_offset() {
    let spec = r##"{
      "data": {"values": [{"a": 1, "b": 2, "c": "x"}, {"a": 4, "b": 8, "c": "y"}]},
      "mark": "bar",
      "encoding": {
        "x": {"field": "c", "type": "nominal", "scale": {"paddingInner": 0.3, "paddingOuter": 0.1, "round": true, "reverse": true, "domain": ["y", "x"]}},
        "y": {"field": "b", "type": "quantitative", "stack": "normalize", "scale": {"type": "sqrt", "clamp": true, "nice": 4, "zero": true}},
        "color": {"field": "a", "type": "quantitative", "scale": {"type": "log", "base": 2, "domainMid": 2, "scheme": {"name": "Viridis", "extent": [0.2, 1]}, "interpolate": {"type": "cubehelix-long", "gamma": 2}}},
        "size": {"field": "a", "type": "quantitative", "scale": {"type": "pow", "exponent": 3, "range": [10, 90]}},
        "shape": {"field": "c", "type": "nominal", "scale": {"range": ["square", "diamond"]}},
        "strokeDash": {"field": "c", "type": "nominal", "scale": {"range": [[1, 2], [3, 1]]}}
      }
    }"##;
    let chart = vega_lite::import(spec).unwrap();
    let e = &chart.layers[0].encoding;
    let channel = |encode: Option<&Channel>| encode.cloned().unwrap();
    let x = channel(e.x.as_ref().and_then(Encode::channel));
    assert_eq!(x.scale.padding_inner, Some(0.3));
    assert_eq!(x.scale.padding_outer, Some(0.1));
    assert!(x.scale.round);
    assert_eq!(x.scale.reverse, Some(true));
    assert_eq!(
        x.scale.domain,
        Some(Domain::Values(vec!["y".into(), "x".into()]))
    );
    let y = channel(e.y.as_ref().and_then(Encode::channel));
    assert_eq!(y.stack, Stack::Normalize);
    assert_eq!(y.scale.kind, Some(ScaleType::Sqrt));
    assert!(y.scale.clamp);
    assert_eq!(y.scale.nice, Some(Nice::Ticks(4.0)));
    let color = channel(e.color.as_ref().and_then(Encode::channel));
    assert_eq!(color.scale.kind, Some(ScaleType::Log));
    assert_eq!(color.scale.base, Some(2.0));
    assert_eq!(color.scale.domain_mid, Some(2.0));
    let scheme = color.scale.scheme.unwrap();
    assert!(matches!(&scheme.palette, Palette::Named(named) if named.name == "viridis"));
    assert_eq!(scheme.extent, Some([0.2, 1.0]));
    assert_eq!(
        color.scale.interpolate,
        Some(interpolate::Interpolate::CubehelixLong { gamma: 2.0 })
    );
    let size = channel(e.size.as_ref().and_then(Encode::channel));
    assert_eq!(size.scale.exponent, Some(3.0));
    assert_eq!(size.scale.range, Some(Range::Numbers(vec![10.0, 90.0])));
    let shape = channel(e.shape.as_ref().and_then(Encode::channel));
    assert_eq!(
        shape.scale.range,
        Some(Range::Shapes(vec![Shape::Square, Shape::Diamond]))
    );
    let dash = channel(e.stroke_dash.as_ref().and_then(Encode::channel));
    assert_eq!(
        dash.scale.range,
        Some(Range::Dashes(vec![vec![1.0, 2.0], vec![3.0, 1.0]]))
    );
    // A named range, and `null` for no stack.
    let named = spec
        .replace(r#""stack": "normalize""#, r#""stack": null"#)
        .replace(
            r#""scheme": {"name": "Viridis", "extent": [0.2, 1]}"#,
            r#""range": "diverging""#,
        );
    let chart = vega_lite::import(&named).unwrap();
    let e = &chart.layers[0].encoding;
    assert_eq!(
        channel(e.y.as_ref().and_then(Encode::channel)).stack,
        Stack::Off
    );
    assert_eq!(
        channel(e.color.as_ref().and_then(Encode::channel))
            .scale
            .range,
        Some(Range::Named(NamedRange::Diverging))
    );
}

#[test]
fn a_scale_reads_its_ends_intervals_lists_and_null() {
    let spec = r##"{
      "data": {"values": [{"t": "2024-01-03", "v": 2, "c": "red", "d": "blue"}]},
      "mark": "point",
      "encoding": {
        "x": {"field": "t", "type": "temporal", "scale": {"nice": {"interval": "month", "step": 3}, "domainMin": "2023-12-01"}},
        "y": {"field": "v", "type": "quantitative", "scale": {"rangeMin": 4, "rangeMax": 100, "domainMax": 9}},
        "color": {"field": "c", "type": "nominal", "scale": null},
        "fill": {"field": "c", "type": "nominal", "scale": {"range": {"field": "d"}}},
        "stroke": {"field": "v", "type": "quantitative", "scale": {"scheme": ["white", "black"]}}
      }
    }"##;
    let chart = vega_lite::import(spec).unwrap();
    let e = &chart.layers[0].encoding;
    let channel = |encode: Option<&Channel>| encode.cloned().unwrap();
    let x = channel(e.x.as_ref().and_then(Encode::channel));
    assert!(matches!(
        x.scale.nice,
        Some(Nice::Interval { step: 3.0, .. })
    ));
    assert_eq!(x.scale.domain_min, time::parse("2023-12-01"));
    let y = channel(e.y.as_ref().and_then(Encode::channel));
    assert_eq!(
        (y.scale.range_min, y.scale.range_max),
        (Some(4.0), Some(100.0))
    );
    assert_eq!(y.scale.domain_max, Some(9.0));
    let color = channel(e.color.as_ref().and_then(Encode::channel));
    assert_eq!(color.scale.kind, Some(ScaleType::Identity));
    let fill = channel(e.fill.as_ref().and_then(Encode::channel));
    assert_eq!(fill.scale.range, Some(Range::Field("d".into())));
    // The range field is read with the encoded fields.
    assert!(chart.layers[0].data.column("d").is_some());
    let stroke = channel(e.stroke.as_ref().and_then(Encode::channel));
    assert!(
        matches!(&stroke.scale.scheme.unwrap().palette, Palette::Colors(colors) if colors.len() == 2)
    );
}

#[test]
fn derived_fields_take_vega_lites_names_and_titles() {
    let spec = r#"{
      "data": {"values": [
        {"d": "2020-01-05", "v": 1, "w": 4}, {"d": "2020-03-09", "v": 7, "w": 2},
        {"d": "2021-03-01", "v": 2, "w": 9}
      ]},
      "layer": [
        {"mark": "bar", "encoding": {
          "x": {"timeUnit": "yearmonth", "field": "d"},
          "y": {"aggregate": "sum", "field": "v"}}},
        {"mark": "point", "encoding": {
          "x": {"timeUnit": "utcyearmonth", "field": "d"},
          "y": {"aggregate": "count"},
          "size": {"aggregate": {"argmax": "v"}, "field": "w", "type": "quantitative"},
          "opacity": {"aggregate": "q1", "field": "w"}}}
      ]
    }"#;
    let chart = vega_lite::import(spec).unwrap();
    let channels = |layer: usize| -> Vec<(String, Option<String>, Kind)> {
        chart.layers[layer]
            .encoding
            .channels()
            .iter()
            .map(|c| {
                let Source::Field(field) = &c.source else {
                    panic!()
                };
                (
                    field.to_string(),
                    c.title.as_ref().map(|t| t.to_string()),
                    c.kind,
                )
            })
            .collect()
    };
    let named = |field: &str, title: Option<&str>, kind| {
        (field.to_string(), title.map(str::to_string), kind)
    };
    assert_eq!(
        channels(0),
        [
            named("yearmonth_d", Some("d (year-month)"), Kind::Temporal),
            named("sum_v", Some("Sum of v"), Kind::Quantitative),
            named("yearmonth_d_end", None, Kind::Temporal),
        ]
    );
    assert_eq!(
        channels(1),
        [
            named("utcyearmonth_d", Some("d (year-month)"), Kind::Temporal),
            named("__count", Some("Count of Records"), Kind::Quantitative),
            named("q1_w", Some("Q1 of w"), Kind::Quantitative),
            named("argmax_v.w", Some("w for max v"), Kind::Quantitative),
        ]
    );
    let Some(Column::Number(best)) = chart.layers[1].data.column("argmax_v.w") else {
        panic!()
    };
    assert_eq!(best[..], [4.0, 2.0, 9.0]);
}

#[test]
fn a_binned_axis_ticks_each_bins_edge_and_a_bar_spans_its_bin() {
    let spec = r#"{
      "data": {"values": [{"r": 1.2}, {"r": 2.5}, {"r": 2.7}, {"r": 6.1}]},
      "mark": "bar",
      "encoding": {"x": {"bin": {"maxbins": 4}, "field": "r"}, "y": {"aggregate": "count"}}
    }"#;
    let chart = vega_lite::import(spec).unwrap();
    let x = chart.layers[0]
        .encoding
        .x
        .as_ref()
        .and_then(Encode::channel)
        .unwrap();
    assert_eq!(x.source, Source::Field("bin_maxbins_4_r".into()));
    assert_eq!(x.title.as_deref(), Some("r (binned)"));
    assert_eq!(
        x.scale.bins,
        Some(Bins {
            start: 0.0,
            stop: 8.0,
            step: 2.0
        })
    );
    let plan = laid_out(&chart);
    let bottom = plan.axes.iter().find(|a| a.side == Side::Bottom).unwrap();
    let ticks: Vec<&str> = bottom.labels.iter().map(|l| l.text.as_ref()).collect();
    // Vega's precision for a tick to each ten pixels, as many as the bins.
    assert_eq!(ticks, ["0.0", "2.0", "4.0", "6.0", "8.0"]);
    // The domain line, then a tick at each edge.
    let edges: Vec<f32> = bottom.rules[1..].iter().map(|r| r.from[0]).collect();
    let chart::plan::Marks::Bars(bars) = &plan.series[0].marks else {
        panic!()
    };
    for bar in bars {
        assert!(edges.iter().any(|e| (e - bar.rect.x).abs() <= 0.5));
        assert!((bar.rect.w - (edges[1] - edges[0])).abs() <= 1.0);
    }
}

#[test]
fn an_unaggregated_domain_spans_the_values_before_aggregation() {
    let spec = r#"{
      "data": {"values": [
        {"g": "a", "v": 0}, {"g": "a", "v": 100}, {"g": "b", "v": 10}, {"g": "b", "v": 20}
      ]},
      "mark": "point",
      "encoding": {
        "x": {"field": "g", "type": "nominal"},
        "y": {"aggregate": "mean", "field": "v", "type": "quantitative",
              "scale": {"domain": "unaggregated"}}
      }
    }"#;
    let chart = vega_lite::import(spec).unwrap();
    let data = &chart.layers[0].data;
    assert_eq!(data.numbers("min_v"), [0.0, 10.0]);
    assert_eq!(data.numbers("max_v"), [100.0, 20.0]);
    let left = |chart: &chart::Chart| {
        let plan = laid_out(chart);
        let axis = plan
            .axes
            .into_iter()
            .find(|a| a.side == Side::Left)
            .unwrap();
        axis.labels.last().unwrap().text.to_string()
    };
    assert_eq!(left(&chart), "100");
    // A count's domain is its own, as Vega-Lite drops `unaggregated` there.
    let distinct = vega_lite::import(&spec.replace("mean", "distinct")).unwrap();
    assert_eq!(left(&distinct), "2.0");
}

#[test]
fn a_position_offset_angle_or_dash_with_no_scale_is_drawn_as_it_is() {
    let plan = |spec: &str| {
        let chart = vega_lite::import(spec).unwrap();
        laid_out(&chart)
    };
    let bars = |plan: &chart::Plan| -> Vec<chart::plan::Bar> {
        plan.series
            .iter()
            .flat_map(|series| match &series.marks {
                chart::plan::Marks::Bars(bars) => bars.clone(),
                _ => Vec::new(),
            })
            .collect()
    };

    // A bar runs from the plot's top to its value in pixels, with no axis.
    let unscaled = plan(
        r#"{
      "data": {"values": [{"k": "a", "v": 30}, {"k": "b", "v": 80}]},
      "mark": "bar",
      "encoding": {"x": {"field": "k", "type": "nominal"},
                   "y": {"field": "v", "type": "quantitative", "scale": null}}
    }"#,
    );
    let heights: Vec<f32> = bars(&unscaled).iter().map(|bar| bar.rect.h).collect();
    assert_eq!(heights, [30.0, 80.0]);
    assert!(
        bars(&unscaled)
            .iter()
            .all(|bar| bar.rect.y == unscaled.plot.y)
    );
    assert!(unscaled.axes.iter().all(|axis| axis.side != Side::Left));

    // A bar runs from its band's start to the offset.
    let offset = plan(
        r#"{
      "data": {"values": [{"k": "a", "v": 30, "w": 5}, {"k": "b", "v": 80, "w": 15}]},
      "mark": "bar",
      "encoding": {"x": {"field": "k", "type": "nominal"},
                   "y": {"field": "v", "type": "quantitative"},
                   "xOffset": {"field": "w", "type": "quantitative", "scale": null}}
    }"#,
    );
    let widths: Vec<f32> = bars(&offset).iter().map(|bar| bar.rect.w).collect();
    assert_eq!(widths, [5.0, 15.0]);

    // Slices stack their angles in radians.
    let pie = plan(
        r#"{
      "data": {"values": [{"k": "a", "v": 1}, {"k": "b", "v": 2}]},
      "mark": "arc",
      "encoding": {"theta": {"field": "v", "type": "quantitative", "scale": null},
                   "color": {"field": "k", "type": "nominal"}}
    }"#,
    );
    let angles: Vec<[f32; 2]> = pie
        .series
        .iter()
        .flat_map(|series| match &series.marks {
            chart::plan::Marks::Slices(slices) => slices.iter().map(|s| [s.start, s.end]).collect(),
            _ => Vec::new(),
        })
        .collect();
    assert_eq!(angles, [[0.0, 1.0], [1.0, 3.0]]);

    // A dash is each row's lengths.
    let dashed = plan(
        r#"{
      "data": {"values": [
        {"x": 1, "y": 1, "d": [4, 2]}, {"x": 2, "y": 2, "d": [4, 2]},
        {"x": 1, "y": 2, "d": [1, 1]}, {"x": 2, "y": 3, "d": [1, 1]}
      ]},
      "mark": "line",
      "encoding": {"x": {"field": "x", "type": "quantitative"},
                   "y": {"field": "y", "type": "quantitative"},
                   "strokeDash": {"field": "d", "type": "nominal", "scale": null}}
    }"#,
    );
    let dashes: Vec<Vec<f32>> = dashed
        .series
        .iter()
        .map(|series| series.style.dash.as_deref().unwrap_or_default().to_vec())
        .collect();
    assert_eq!(dashes, [vec![4.0, 2.0], vec![1.0, 1.0]]);
}

#[test]
fn a_histogram_stacks_each_bin_by_colour() {
    let chart = vega_lite::import(
        r#"{
      "data": {"values": [
        {"b": 1, "g": "p"}, {"b": 2, "g": "q"}, {"b": 2, "g": "p"}, {"b": 6, "g": "q"},
        {"b": 7, "g": "q"}, {"b": 7, "g": "p"}, {"b": 3, "g": "p"}
      ]},
      "mark": "bar",
      "encoding": {"x": {"bin": {"maxbins": 4}, "field": "b"},
                   "y": {"aggregate": "count"},
                   "color": {"field": "g"}}
    }"#,
    )
    .unwrap();
    let plan = laid_out(&chart);
    // Vega's stack: q from 0 and p on it, in each bin.
    let unit = {
        let left = plan.axes.iter().find(|a| a.side == Side::Left).unwrap();
        // The domain line, then a tick at each value.
        (left.rules[1].from[1] - left.rules[3].from[1]) as f64
    };
    let mut bars: Vec<(String, f32, f64, f64)> = Vec::new();
    for series in &plan.series {
        let chart::plan::Marks::Bars(marks) = &series.marks else {
            panic!()
        };
        for bar in marks {
            let bottom = (plan.plot.bottom() - bar.rect.bottom()) as f64 / unit;
            let top = (plan.plot.bottom() - bar.rect.y) as f64 / unit;
            let name = series.name.as_deref().unwrap_or_default().to_string();
            bars.push((name, bar.rect.x, bottom.round(), top.round()));
        }
    }
    bars.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)));
    let stacks: Vec<(&str, f64, f64)> = bars
        .iter()
        .map(|(n, _, b, t)| (n.as_str(), *b, *t))
        .collect();
    assert_eq!(
        stacks,
        [
            ("p", 0.0, 1.0),
            ("q", 0.0, 1.0),
            ("p", 1.0, 3.0),
            ("q", 0.0, 2.0),
            ("p", 2.0, 3.0)
        ]
    );
}

#[test]
fn a_binned_colour_size_or_opacity_lists_its_bins() {
    let legend = |channel: &str| -> Vec<String> {
        let spec = r#"{
          "data": {"values": [{"a": 1, "b": 3}, {"a": 2, "b": 7}, {"a": 3, "b": 12}, {"a": 4, "b": 18}]},
          "mark": "point",
          "encoding": {"x": {"field": "a", "type": "quantitative"},
                       "y": {"field": "a", "type": "quantitative"},
                       "CHANNEL": {"bin": true, "field": "b", "type": "quantitative"}}
        }"#
        .replace("CHANNEL", channel);
        let chart = vega_lite::import(&spec).unwrap();
        let plan = laid_out(&chart);
        let legend = &plan.legends[0];
        legend
            .swatches
            .iter()
            .map(|swatch| &swatch.label)
            .chain(&legend.labels)
            .map(|label| label.text.to_string())
            .collect()
    };
    let bins = ["0 – 5", "5 – 10", "10 – 15", "15 – 20"];
    // Colour's steps are a discrete gradient, labelled where each but the
    // first starts.
    assert_eq!(legend("color"), ["5", "10", "15"]);
    assert_eq!(legend("size"), bins);
    assert_eq!(legend("opacity"), bins);
    // Shape's scale is ordinal, over where each bin starts.
    assert_eq!(legend("shape"), ["0", "5", "10", "15"]);
}

#[test]
fn invalid_marks_follow_the_mark_and_domains_the_config_alone() {
    let mode = |mark: &str, config: &str| {
        let spec = format!(
            r#"{{"data": {{"values": [{{"x": 1, "y": 2}}, {{"x": 2, "y": null}}]}},
              "mark": {mark}, "config": {config},
              "encoding": {{"x": {{"field": "x", "type": "quantitative"}},
                            "y": {{"field": "y", "type": "quantitative"}}}}}}"#
        );
        vega_lite::import(&spec).map(|chart| chart.layers[0].properties.invalid)
    };
    let mark = |kind: &str, invalid: &str| format!(r#"{{"type": "{kind}", "invalid": {invalid}}}"#);
    // Each is Vega-Lite 6.5's: a mark's own mode parts no domain from its
    // marks.
    assert_eq!(
        mode(&mark("line", r#""break-paths-filter-domains""#), "{}"),
        Some(None)
    );
    assert_eq!(
        mode(&mark("point", r#""break-paths-show-domains""#), "{}"),
        Some(None)
    );
    assert_eq!(mode(&mark("bar", "null"), "{}"), Some(Some(Invalid::Show)));
    assert_eq!(
        mode(&mark("line", r#""filter""#), "{}"),
        Some(Some(Invalid::Filter))
    );
    let config = |invalid: &str| format!(r#"{{"mark": {{"invalid": {invalid}}}}}"#);
    assert_eq!(
        mode(r#""line""#, &config(r#""break-paths-filter-domains""#)),
        Some(Some(Invalid::BreakPathsFilterDomains))
    );
    assert_eq!(
        mode(r#""point""#, &config(r#""break-paths-show-domains""#)),
        Some(Some(Invalid::BreakPathsShowDomains))
    );
    assert_eq!(
        mode(r#""point""#, &config(r#""break-paths-show-path-domains""#)),
        Some(None)
    );
    // `config.<mark>` over `config.mark`.
    assert_eq!(
        mode(
            r#""line""#,
            r#"{"line": {"invalid": "filter"}, "mark": {"invalid": "show"}}"#
        ),
        Some(Some(Invalid::Filter))
    );
    assert_eq!(mode(&mark("line", r#""sometimes""#), "{}"), None);
}
