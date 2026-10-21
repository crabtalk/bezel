//! Each `tests/transform/<case>.vl.json` imported and compared with the data
//! Vega computes for it, `<case>.vega.json` (see `tests/vega`).

use chart::{
    Column, Data, Transform,
    transform::{Density, Quantile, Regression},
    vega_lite,
};
use serde_json::{Map, Value};
use std::{fs, path::Path};

/// Relative to the larger of 1 and the expected value.
const TOLERANCE: f64 = 1e-9;

fn case(name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/transform");
    let read = |file: String| fs::read_to_string(dir.join(file)).expect("a fixture");
    let spec = read(format!("{name}.vl.json"));
    let expected: Vec<Vec<Map<String, Value>>> =
        serde_json::from_str(&read(format!("{name}.vega.json"))).expect("fixture JSON");
    let chart = vega_lite::import(&spec).expect("imports");
    assert_eq!(chart.layers.len(), expected.len(), "{name}: layers");
    for (i, (layer, rows)) in chart.layers.iter().zip(&expected).enumerate() {
        assert_eq!(layer.data.len(), rows.len(), "{name} layer {i}: rows");
        for (field, column) in layer.data.fields() {
            for (row, want) in rows.iter().enumerate() {
                let want = want.get(field.as_ref()).unwrap_or(&Value::Null);
                let at = format!("{name} layer {i} row {row} `{field}`");
                match column {
                    Column::Number(values) => match want.as_f64() {
                        Some(want) => assert!(
                            (values[row] - want).abs() <= TOLERANCE * want.abs().max(1.0),
                            "{at}: {} is not {want}",
                            values[row]
                        ),
                        None => assert!(values[row].is_nan(), "{at}: {} is not null", values[row]),
                    },
                    Column::Text(text) => {
                        assert_eq!(text.get(row), want.as_str().unwrap_or(""), "{at}")
                    }
                }
            }
        }
    }
}

#[test]
fn density_shares_even_samples_across_groups() {
    case("density");
}

#[test]
fn independent_density_samples_each_group_adaptively_as_counts() {
    case("density_independent");
}

#[test]
fn cumulative_density_takes_a_bandwidth_extent_steps_and_names() {
    case("density_cumulative");
}

#[test]
fn loess_smooths_each_group_and_averages_repeated_x() {
    case("loess");
}

#[test]
fn loess_takes_a_bandwidth_and_names() {
    case("loess_bandwidth");
}

#[test]
fn regression_fits_each_method_in_its_layer() {
    case("regression");
}

#[test]
fn quantile_steps_through_each_group() {
    case("quantile");
}

#[test]
fn quantile_takes_probabilities_and_names() {
    case("quantile_probs");
}

#[test]
fn a_layer_runs_its_own_transforms_after_the_top_levels() {
    case("layer");
}

#[test]
fn an_op_the_pipeline_lacks_is_none() {
    let spec = |transform: &str| {
        format!(
            r#"{{"data": {{"values": [{{"x": 1, "y": 2}}, {{"x": 2, "y": 3}}, {{"x": 3, "y": 5}}]}},
              "transform": [{transform}],
              "mark": "line",
              "encoding": {{"x": {{"field": "x", "type": "quantitative"}}, "y": {{"field": "y", "type": "quantitative"}}}}}}"#
        )
    };
    assert!(vega_lite::import(&spec(r#"{"regression": "y", "on": "x"}"#)).is_some());
    assert!(vega_lite::import(&spec("")).is_some());
    for lacked in [
        r#"{"filter": "datum.x > 1"}"#,
        r#"{"calculate": "datum.x * 2", "as": "x2"}"#,
        r#"{"aggregate": [{"op": "sum", "field": "y", "as": "y"}], "groupby": ["x"]}"#,
        r#"{"regression": "y", "on": "x", "params": true}"#,
        r#"{"regression": "y", "on": "x", "method": "constant"}"#,
        r#"{"density": "x", "filter": "datum.x > 1"}"#,
        r#"{"quantile": "x", "probs": "all"}"#,
    ] {
        assert!(
            vega_lite::import(&spec(lacked)).is_none(),
            "{lacked} is not read"
        );
        let layered = spec("").replace(
            r#""mark": "line""#,
            &format!(r#""layer": [{{"mark": "line", "transform": [{lacked}]}}]"#),
        );
        assert!(
            vega_lite::import(&layered).is_none(),
            "{lacked} in a layer is not read"
        );
    }
}

#[test]
fn a_pipeline_runs_over_data_in_order() {
    let data = Data::new()
        .text("g", ["a", "a", "b", "b", "b"])
        .number("x", [1.0, 2.0, 3.0, 4.0, 5.0]);
    let mut quantile = Quantile::new("x");
    quantile.probs = Some(vec![0.0, 0.5, 1.0]);
    let pipeline = [
        Transform::Quantile(quantile),
        Transform::Regression(Regression::new("value", "prob")),
    ];
    let out = data.clone().transform(&pipeline);
    assert_eq!(out.numbers("prob"), [0.0, 1.0]);
    let value = out.numbers("value");
    assert!((value[0] - 1.0).abs() < TOLERANCE && (value[1] - 5.0).abs() < TOLERANCE);

    let mut density = Density::new("x");
    density.groupby = vec!["g".into()];
    density.steps = Some(4);
    let out = data.transform(&[Transform::Density(density)]);
    let fields: Vec<&str> = out.fields().map(|(name, _)| name.as_ref()).collect();
    assert_eq!(fields, ["g", "value", "density"]);
    assert_eq!(out.len(), 10);
}
