//! Each `tests/transform/<case>.vl.json` imported and compared with the data
//! Vega computes for it, `<case>.vega.json`, and with the domains of its
//! discrete scales, `<case>.domains.json`, written by `bezel-vega-lite`'s
//! `tests/reference/fixtures.mjs`.

use chart::{
    Chart, Column, Data, Transform,
    model::Side,
    plan,
    plan::Metrics,
    transform::{Density, Quantile, Regression},
    vega_lite,
};
use d3::format::number::Number as JsNumber;
use serde_json::{Map, Value};
use std::{fs, path::Path};
use vega::{statistics::Random, value::Value as JsValue};

/// Relative to the larger of 1 and the expected value.
const TOLERANCE: f64 = 1e-9;
/// Text 10 high on lines 10 apart.
const METRICS: Metrics = Metrics {
    size: 10.0,
    line: 10.0,
};

fn case(name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/transform");
    let read = |file: String| fs::read_to_string(dir.join(file)).expect("a fixture");
    let spec = read(format!("{name}.vl.json"));
    let expected: Vec<Vec<Map<String, Value>>> =
        serde_json::from_str(&read(format!("{name}.vega.json"))).expect("fixture JSON");
    let chart = vega_lite::import(&spec).expect("imports");
    if let Ok(domains) = fs::read_to_string(dir.join(format!("{name}.domains.json"))) {
        let domains: Map<String, Value> = serde_json::from_str(&domains).expect("fixture JSON");
        for (scale, domain) in domains {
            let domain: Vec<String> = domain
                .as_array()
                .expect("a domain")
                .iter()
                .map(|value| match value {
                    Value::String(text) => text.clone(),
                    Value::Null => "null".into(),
                    value => JsNumber(value.as_f64().expect("a number")).to_string(),
                })
                .collect();
            assert_eq!(
                order(&chart, &scale),
                domain,
                "{name}: the domain of `{scale}`"
            );
        }
    }
    // The rows as drawn, without those the layers' invalid data modes leave
    // out.
    let plan = plan::plan(&chart, [100_000.0, 100_000.0], METRICS, &mut |_, _| 1.0).expect("draws");
    let drawn = plan.chart();
    assert_eq!(drawn.layers.len(), expected.len(), "{name}: layers");
    for (i, (layer, rows)) in drawn.layers.iter().zip(&expected).enumerate() {
        assert_eq!(layer.data.len(), rows.len(), "{name} layer {i}: rows");
        for (field, column) in layer.data.fields() {
            for (row, want) in rows.iter().enumerate() {
                let want = lookup(want, field).unwrap_or(&Value::Null);
                let at = format!("{name} layer {i} row {row} `{field}`");
                match column {
                    Column::Number(values) => match want.as_f64() {
                        Some(want) => assert!(
                            (values[row] - want).abs() <= TOLERANCE * want.abs().max(1.0),
                            "{at}: {} is not {want}",
                            values[row]
                        ),
                        // JSON writes an infinity as null.
                        None => assert!(
                            !values[row].is_finite(),
                            "{at}: {} is not null",
                            values[row]
                        ),
                    },
                    Column::Text(text) => {
                        assert_eq!(text.get(row).map(AsRef::as_ref), want.as_str(), "{at}")
                    }
                    Column::Value(values) => assert!(
                        same(&values[row], want),
                        "{at}: {} is not {want}",
                        values[row]
                    ),
                }
            }
        }
    }
}

/// Whether a JavaScript value is the JSON Vega wrote of it: undefined as
/// null, numbers to [`TOLERANCE`].
fn same(value: &JsValue, want: &Value) -> bool {
    match (value, want) {
        (JsValue::Undefined | JsValue::Null, Value::Null) => true,
        (JsValue::Bool(a), Value::Bool(b)) => a == b,
        (JsValue::Number(a), Value::Number(b)) => {
            let b = b.as_f64().expect("a number");
            (a - b).abs() <= TOLERANCE * b.abs().max(1.0)
        }
        (JsValue::Number(a), Value::Null) => !a.is_finite(),
        (JsValue::String(a), Value::String(b)) => **a == **b,
        (JsValue::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
        }
        (JsValue::Object(a), Value::Object(b)) => {
            let defined = a.iter().filter(|(_, v)| !matches!(v, JsValue::Undefined));
            defined.clone().count() == b.len()
                && defined
                    .into_iter()
                    .all(|(k, v)| b.get(&**k).is_some_and(|w| same(v, w)))
        }
        _ => false,
    }
}

/// `field` of a Vega row, a path of dotted names and bracketed indices
/// stepping into an object where the row has no field of that name.
fn lookup<'a>(row: &'a Map<String, Value>, field: &str) -> Option<&'a Value> {
    row.get(field).or_else(|| {
        let path = field.replace('[', ".").replace(']', "");
        let (object, rest) = path.split_once('.')?;
        rest.split('.')
            .try_fold(row.get(object)?, |value, key| match value {
                Value::Array(items) => items.get(key.parse::<usize>().ok()?),
                value => value.get(key),
            })
    })
}

/// The values of a discrete scale in the order the plan draws them: the
/// bands along `x` or down `y`, or the colours.
fn order(chart: &Chart, scale: &str) -> Vec<String> {
    let plan = plan::plan(chart, [100_000.0, 100_000.0], METRICS, &mut |_, _| 1.0).expect("draws");
    match scale {
        "color" => plan
            .colors
            .iter()
            .filter_map(|color| Some(color.name.clone()?.to_string()))
            .collect(),
        "x" | "y" => {
            let side = if scale == "x" {
                Side::Bottom
            } else {
                Side::Left
            };
            let axis = plan
                .axes
                .iter()
                .find(|axis| axis.side == side)
                .expect("an axis");
            axis.labels
                .iter()
                .map(|label| label.text.to_string())
                .collect()
        }
        _ => panic!("no scale `{scale}`"),
    }
}

#[test]
fn aggregate_computes_every_op_per_group_as_vega_does() {
    case("aggregate");
}

#[test]
fn layers_union_their_domains_keeping_a_count_min_or_max_sort() {
    case("layer_union");
}

#[test]
fn layers_over_the_same_rows_share_one_domain_and_its_sort() {
    case("layer_shared");
}

#[test]
fn min_and_max_over_text_compare_strings() {
    case("aggregate_text");
}

#[test]
fn dates_group_by_the_second() {
    case("aggregate_dates");
}

#[test]
fn confidence_intervals_draw_from_one_generator_across_aggregates() {
    case("aggregate_random");
}

#[test]
fn an_encoding_aggregates_over_its_other_fields_and_sorts_by_another_channel() {
    case("aggregate_encoding");
}

#[test]
fn bin_takes_every_parameter() {
    case("bin");
}

#[test]
fn a_histogram_bins_and_counts() {
    case("histogram");
}

#[test]
fn an_ordinal_bin_reads_its_range_as_text_sorted_by_count() {
    case("histogram_ordinal");
}

#[test]
fn time_units_truncate_every_unit_local_and_utc() {
    case("timeunit");
}

#[test]
fn an_encoded_time_unit_spans_its_step_on_a_bar() {
    case("timeunit_encoding");
}

#[test]
fn a_binned_time_unit_ends_a_step_on() {
    case("binned_timeunit");
}

#[test]
fn a_sort_by_an_aggregate_orders_a_domain() {
    case("sort");
}

#[test]
fn a_stacked_sort_sums_and_a_count_needs_no_field() {
    case("sort_stacked");
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
    let out = data.clone().transform(&pipeline, &mut Random::default());
    assert_eq!(out.numbers("prob"), [0.0, 1.0]);
    let value = out.numbers("value");
    assert!((value[0] - 1.0).abs() < TOLERANCE && (value[1] - 5.0).abs() < TOLERANCE);

    let mut density = Density::new("x");
    density.groupby = vec!["g".into()];
    density.steps = Some(4);
    let out = data.transform(&[Transform::Density(density)], &mut Random::default());
    let fields: Vec<&str> = out.fields().map(|(name, _)| name.as_ref()).collect();
    assert_eq!(fields, ["g", "value", "density"]);
    assert_eq!(out.len(), 10);
}

#[test]
fn a_layer_bins_its_encoding_before_its_own_transforms() {
    case("layer_bin");
}

#[test]
fn a_boxplot_draws_outliers_whiskers_box_and_median() {
    case("boxplot");
}

#[test]
fn a_min_max_boxplot_whiskers_each_group_and_bins_its_other_axis() {
    case("boxplot_minmax");
}

#[test]
fn a_boxplot_layer_takes_its_extent_and_parts() {
    case("boxplot_layer");
}

#[test]
fn an_errorbar_shares_one_bootstrap_with_a_layer_aggregating_alike() {
    case("errorbar");
}

#[test]
fn errorbars_take_every_extent_and_merge_as_vega_lite_merges_them() {
    case("errorbar_extents");
}

#[test]
fn an_errorbar_reads_bounds_or_errors_given_already() {
    case("errorbar_preaggregated");
}

#[test]
fn an_errorband_bounds_an_area_with_borders() {
    case("errorband");
}

#[test]
fn a_one_axis_errorband_is_a_rect_with_rules() {
    case("errorband_1d");
}

#[test]
fn layers_aggregating_by_the_same_fields_merge_their_aggregates() {
    case("aggregate_merged");
}

#[test]
fn layers_share_the_top_levels_transforms() {
    case("aggregate_shared");
}

#[test]
fn an_errorbar_reads_an_error_about_each_value() {
    case("errorbar_error");
}

#[test]
fn regression_params_give_each_groups_coefficients_and_r_squared() {
    case("regression_params");
}

#[test]
fn the_values_op_gives_each_groups_rows() {
    case("aggregate_values");
}

#[test]
fn nested_json_is_kept_and_read_by_path() {
    case("nested");
}

#[test]
fn a_missing_field_groups_apart_from_null() {
    case("missing");
}

#[test]
fn invalid_values_leave_marks_and_domains_by_default() {
    case("invalid_filter");
}

#[test]
fn invalid_values_are_drawn_where_shown() {
    case("invalid_show");
}

#[test]
fn config_gives_a_mark_its_invalid_mode() {
    case("invalid_config");
}

#[test]
fn paths_break_at_invalid_values_or_join_round_them() {
    case("invalid_paths");
}

#[test]
fn null_is_a_group_and_a_category() {
    case("aggregate_null");
}
