//! d3-format's output for every case in `d3/format.json`, which
//! `d3/fixtures.mjs` writes from the pinned d3-format.

use js::{
    format::{self, NumberFormat, Specifier},
    number::Number,
};
use serde_json::Value;

fn cases(key: &str) -> Vec<Vec<Value>> {
    let all: Value = serde_json::from_str(include_str!("d3/format.json")).expect("fixture");
    serde_json::from_value(all[key].clone()).expect("cases")
}

fn text(value: &Value) -> &str {
    value.as_str().expect("text")
}

fn number(value: &Value) -> f64 {
    Number::from(text(value)).0
}

fn assert_none(failures: Vec<String>, total: usize) {
    assert!(
        failures.is_empty(),
        "{} of {total} differ:\n{}",
        failures.len(),
        failures[..failures.len().min(40)].join("\n")
    );
}

#[test]
fn number_formats() {
    let cases = cases("number");
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| {
            let f: NumberFormat = text(&case[0]).parse().expect("a specifier");
            let got = f.format(number(&case[1]));
            (got != text(&case[2])).then(|| format!("{case:?}: {got:?}"))
        })
        .collect();
    assert_none(failures, cases.len());
}

#[test]
fn prefix_formats() {
    let cases = cases("prefix");
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| {
            let specifier: Specifier = text(&case[0]).parse().expect("a specifier");
            let f = NumberFormat::prefix(specifier, number(&case[1]));
            let got = f.format(number(&case[2]));
            (got != text(&case[3])).then(|| format!("{case:?}: {got:?}"))
        })
        .collect();
    assert_none(failures, cases.len());
}

#[test]
fn precisions() {
    let cases = cases("precision");
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| {
            let step = number(&case[1]);
            let got = match text(&case[0]) {
                "fixed" => format::precision_fixed(step),
                "prefix" => format::precision_prefix(step, number(&case[2])),
                _ => format::precision_round(step, number(&case[2])),
            };
            let want = case[3].as_u64().map(|p| p as usize);
            (got != want).then(|| format!("{case:?}: {got:?}"))
        })
        .collect();
    assert_none(failures, cases.len());
}

#[test]
fn specifiers() {
    let cases = cases("specifier");
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| {
            let got = text(&case[0])
                .parse::<Specifier>()
                .ok()
                .map(|s| s.to_string());
            let want = case[1].as_str().map(str::to_string);
            (got != want).then(|| format!("{case:?}: {got:?}"))
        })
        .collect();
    assert_none(failures, cases.len());
}

#[test]
fn invalid_specifier_says_which() {
    let error = "xx".parse::<NumberFormat>().unwrap_err();
    assert_eq!(error.to_string(), "invalid format: xx");
}

/// vega-format's `formatSpan` and `formatFloat`, against what Vega 6.4.0
/// writes.
#[test]
fn spans_and_floats_are_vegas() {
    let span = |start, stop, count, specifier: Option<&str>, values: &[f64]| -> Vec<String> {
        let specifier = specifier.map(|s| s.parse::<Specifier>().expect("a specifier"));
        let format = NumberFormat::span(start, stop, count, specifier);
        values.iter().map(|&v| format.format(v)).collect()
    };
    assert_eq!(
        span(0.0, 100.0, 10.0, None, &[0.0, 20.0, -40.0, 1000.0]),
        ["0", "20", "\u{2212}40", "1,000"]
    );
    assert_eq!(
        span(0.0, 1.0, 10.0, None, &[0.0, 0.1, -0.3]),
        ["0.0", "0.1", "\u{2212}0.3"]
    );
    assert_eq!(
        span(0.0, 26000.0, 5.0, Some("s"), &[0.0, 10000.0, 20000.0]),
        ["0k", "10k", "20k"]
    );
    assert_eq!(span(0.0, 1.0, 5.0, Some("%"), &[0.0, 0.2]), ["0%", "20%"]);
    assert_eq!(
        span(0.0, 1000.0, 5.0, Some("e"), &[0.0, 200.0]),
        ["0e+0", "2e+2"]
    );
    let float = NumberFormat::float(None);
    let floats: Vec<String> = [1.0, 1000.0, 0.5, 1234567.891]
        .iter()
        .map(|&v| float.format(v))
        .collect();
    assert_eq!(floats, ["1", "1,000", "0.5", "1,234,567.891"]);
    let fixed = NumberFormat::float(Some(".2f".parse().expect("a specifier")));
    assert_eq!(fixed.format(1.234), "1.23");
}
