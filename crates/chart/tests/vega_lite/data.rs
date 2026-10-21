use chart::{Chart, Column, time, vega_lite::Spec};

/// A bar chart of `x` against `y` over `data`, with `files` behind its urls.
fn bars(data: &str, x: &str, y: &str, files: &[(&str, &str)]) -> Option<Chart> {
    let spec = format!(
        r#"{{"data": {data}, "mark": "bar", "encoding": {{
          "x": {{"field": "{x}", "type": "nominal"}},
          "y": {{"field": "{y}", "type": "quantitative"}}}}}}"#
    );
    let spec: Spec = spec.parse().expect("spec JSON");
    spec.chart(&|url| {
        files
            .iter()
            .find(|(name, _)| *name == url)
            .map(|(_, text)| *text)
    })
}

fn text(chart: &Chart, field: &str) -> Vec<String> {
    match chart.layers[0].data.column(field) {
        Some(Column::Text(text)) => (0..text.codes.len())
            .map(|row| text.get(row).to_string())
            .collect(),
        other => panic!("`{field}` is not text: {other:?}"),
    }
}

fn numbers(chart: &Chart, field: &str) -> Vec<f64> {
    match chart.layers[0].data.column(field) {
        Some(Column::Number(values)) => values.to_vec(),
        other => panic!("`{field}` is not numbers: {other:?}"),
    }
}

#[test]
fn a_url_is_read_through_the_loader() {
    let data = r#"{"url": "data/a.csv"}"#;
    assert!(bars(data, "k", "v", &[]).is_none());
    let chart = bars(data, "k", "v", &[("data/a.csv", "k,v\nx,1\ny,2.5\n")]).unwrap();
    assert_eq!(text(&chart, "k"), ["x", "y"]);
    assert_eq!(numbers(&chart, "v"), [1.0, 2.5]);
}

#[test]
fn urls_are_each_named_once_in_order() {
    let spec: Spec = r#"{"data": {"url": "b.json"}, "layer": [
        {"mark": "bar"}, {"data": {"url": "a.csv"}, "mark": "line"},
        {"data": {"url": "b.json"}, "mark": "point"}]}"#
        .parse()
        .unwrap();
    assert_eq!(spec.urls(), ["b.json", "a.csv"]);
    assert!("[1, 2]".parse::<Spec>().is_err());
}

#[test]
fn a_urls_extension_names_its_format_unless_format_does() {
    let files = [
        ("t.tsv", "k\tv\nx\t3"),
        ("t.txt", r#"[{"k": "x", "v": 4}]"#),
    ];
    let tsv = bars(r#"{"url": "t.tsv"}"#, "k", "v", &files).unwrap();
    assert_eq!(numbers(&tsv, "v"), [3.0]);
    let json = bars(r#"{"url": "t.txt"}"#, "k", "v", &files).unwrap();
    assert_eq!(numbers(&json, "v"), [4.0]);
    assert!(
        bars(
            r#"{"url": "t.tsv", "format": {"type": "json"}}"#,
            "k",
            "v",
            &files
        )
        .is_none()
    );
    assert!(
        bars(
            r#"{"url": "t.tsv", "format": {"type": "xml"}}"#,
            "k",
            "v",
            &files
        )
        .is_none()
    );
}

#[test]
fn csv_reads_quotes_and_line_breaks_as_d3_dsv_does() {
    let csv = "k,v,w\r\n\"a, \"\"b\"\"\",1,x\r\n\"two\nlines\",2\r\nc,,z";
    let data = format!(
        r#"{{"values": {}, "format": {{"type": "csv"}}}}"#,
        serde_json::to_string(csv).unwrap()
    );
    let chart = bars(&data, "k", "v", &[]).unwrap();
    assert_eq!(text(&chart, "k"), ["a, \"b\"", "two\nlines", "c"]);
    // `""` is 0 to a quantitative channel, as `+""` is.
    assert_eq!(numbers(&chart, "v"), [1.0, 2.0, 0.0]);
}

#[test]
fn csv_fields_stay_strings_unless_parsed() {
    let data = r#"{"values": "k,v\n10,1\n9,2", "format": {"type": "csv"}}"#;
    assert_eq!(text(&bars(data, "k", "v", &[]).unwrap(), "k"), ["10", "9"]);
    let parsed =
        r#"{"values": "k,v\n10,1\n9,2", "format": {"type": "csv", "parse": {"k": "number"}}}"#;
    assert_eq!(
        numbers(&bars(parsed, "k", "v", &[]).unwrap(), "k"),
        [10.0, 9.0]
    );
}

#[test]
fn dsv_takes_one_delimiter_character() {
    let data = r#"{"values": "k|v\nx|5", "format": {"type": "dsv", "delimiter": "|"}}"#;
    assert_eq!(numbers(&bars(data, "k", "v", &[]).unwrap(), "v"), [5.0]);
    for format in [
        r#"{"type": "dsv"}"#,
        r#"{"type": "dsv", "delimiter": "||"}"#,
        r#"{"type": "csv", "delimiter": "|"}"#,
    ] {
        let data = format!(r#"{{"values": "k|v\nx|5", "format": {format}}}"#);
        assert!(bars(&data, "k", "v", &[]).is_none(), "{format}");
    }
}

#[test]
fn json_property_and_fields_are_paths() {
    let data = r#"{"values": {"a": {"b": [{"k": {"x.y": "p"}, "v": [7]}]}},
        "format": {"property": "a.b"}}"#;
    let chart = bars(data, "k.x\\\\.y", "v[0]", &[]).unwrap();
    assert_eq!(text(&chart, "k.x\\.y"), ["p"]);
    assert_eq!(numbers(&chart, "v[0]"), [7.0]);
    let file = [("a.json", r#"{"rows": [{"k": "q", "v": 8}]}"#)];
    let chart = bars(
        r#"{"url": "a.json", "format": {"property": "rows"}}"#,
        "k",
        "v",
        &file,
    )
    .unwrap();
    assert_eq!(text(&chart, "k"), ["q"]);
}

#[test]
fn parse_reads_numbers_booleans_and_dates() {
    let data = r#"{"values": "k,v,d\nfalse,1,Jan 2 2000\n0,x,2000/01/03\nyes,,",
        "format": {"type": "csv", "parse": {"k": "boolean", "d": "date"}}}"#;
    let chart = bars(data, "k", "v", &[]).unwrap();
    assert_eq!(text(&chart, "k"), ["false", "false", "true"]);
    let v = numbers(&chart, "v");
    assert!(v[1].is_nan() && v[0] == 1.0);
    let chart = bars(data, "d", "v", &[]).unwrap();
    let d = numbers(&chart, "d");
    assert_eq!(d[0], time::parse("2000-01-02").unwrap());
    assert_eq!(d[1] - d[0], time::DAY);
    assert!(d[2].is_nan());
}

#[test]
fn a_parse_with_a_time_format_is_none() {
    let data =
        r#"{"values": "k,v\n2000,1", "format": {"parse": {"k": "date:'%Y'"}, "type": "csv"}}"#;
    assert!(bars(data, "k", "v", &[]).is_none());
    let data = r#"{"values": "k,v\nx,1", "format": {"parse": {"k": "decimal"}, "type": "csv"}}"#;
    assert!(bars(data, "k", "v", &[]).is_none());
}

const TOPOLOGY: &str = r#"{"type": "Topology", "arcs": [[[0, 0], [1, 1]]], "objects": {
  "states": {"type": "GeometryCollection", "geometries": [
    {"type": "LineString", "arcs": [0], "id": "s1", "properties": {"name": "One", "pop": 3}},
    {"type": "LineString", "arcs": [0], "id": "s2", "properties": {"name": "Two", "pop": 4}}]},
  "land": {"type": "LineString", "arcs": [0]}}}"#;

#[test]
fn topojson_features_are_rows_of_their_properties() {
    let files = [("us.json", TOPOLOGY)];
    let feature = r#"{"url": "us.json", "format": {"type": "topojson", "feature": "states"}}"#;
    let chart = bars(feature, "properties.name", "properties.pop", &files).unwrap();
    assert_eq!(text(&chart, "properties.name"), ["One", "Two"]);
    assert_eq!(numbers(&chart, "properties.pop"), [3.0, 4.0]);
    let chart = bars(feature, "id", "properties.pop", &files).unwrap();
    assert_eq!(text(&chart, "id"), ["s1", "s2"]);

    let land = r#"{"url": "us.json", "format": {"type": "topojson", "feature": "land"}}"#;
    let chart = bars(land, "type", "v", &files).unwrap();
    assert_eq!(text(&chart, "type"), ["Feature"]);
    let mesh = r#"{"url": "us.json", "format": {"type": "topojson", "mesh": "states"}}"#;
    let chart = bars(mesh, "type", "v", &files).unwrap();
    assert_eq!(text(&chart, "type"), ["MultiLineString"]);

    let missing = r#"{"url": "us.json", "format": {"type": "topojson", "feature": "counties"}}"#;
    assert!(bars(missing, "type", "v", &files).is_none());
    let neither = r#"{"url": "us.json", "format": {"type": "topojson"}}"#;
    assert!(bars(neither, "type", "v", &files).is_none());
}

#[test]
fn a_name_reads_the_top_levels_datasets() {
    let spec = |name: &str| {
        format!(
            r#"{{"datasets": {{"rows": [{{"k": "a", "v": 1}}], "csv": "k,v\nb,2"}},
            "data": {{"name": "{name}"}}, "mark": "bar",
            "encoding": {{"x": {{"field": "k", "type": "nominal"}}}}}}"#
        )
    };
    let chart = |name: &str| spec(name).parse::<Spec>().unwrap().chart(&|_| None);
    assert_eq!(text(&chart("rows").unwrap(), "k"), ["a"]);
    assert!(chart("missing").is_none());
    let csv = spec("csv").replace(
        r#""name": "csv""#,
        r#""name": "csv", "format": {"type": "csv"}"#,
    );
    let csv = csv.parse::<Spec>().unwrap().chart(&|_| None).unwrap();
    assert_eq!(text(&csv, "k"), ["b"]);
}

#[test]
fn values_that_are_not_objects_are_data() {
    let chart = bars(r#"{"values": ["a", "b"]}"#, "data", "v", &[]).unwrap();
    assert_eq!(text(&chart, "data"), ["a", "b"]);
}

#[test]
fn a_sequence_is_d3s_range() {
    let chart = bars(
        r#"{"sequence": {"start": 0, "stop": 1, "step": 0.25, "as": "x"}}"#,
        "k",
        "x",
        &[],
    )
    .unwrap();
    assert_eq!(numbers(&chart, "x"), [0.0, 0.25, 0.5, 0.75]);
    let chart = bars(r#"{"sequence": {"start": 1, "stop": 4}}"#, "k", "data", &[]).unwrap();
    assert_eq!(numbers(&chart, "data"), [1.0, 2.0, 3.0]);
    let empty = bars(r#"{"sequence": {"start": 4, "stop": 1}}"#, "k", "data", &[]).unwrap();
    assert!(empty.layers[0].data.is_empty());
    assert!(bars(r#"{"sequence": {"start": 1}}"#, "k", "data", &[]).is_none());
}

#[test]
fn a_sphere_and_a_graticule_are_one_shape() {
    let sphere = bars(r#"{"sphere": true}"#, "type", "v", &[]).unwrap();
    assert_eq!(text(&sphere, "type"), ["Sphere"]);
    let graticule = bars(r#"{"graticule": {"step": [15, 15]}}"#, "type", "v", &[]).unwrap();
    assert_eq!(text(&graticule, "type"), ["MultiLineString"]);
    assert!(bars(r#"{"graticule": {"steps": 1}}"#, "type", "v", &[]).is_none());
    assert!(bars(r#"{"values": [], "colour": 1}"#, "type", "v", &[]).is_none());
}
