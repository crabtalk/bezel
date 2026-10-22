//! d3-time-format's output for every case in `d3/time_format.json`, which
//! `d3/fixtures.mjs` writes from the pinned d3-time-format with local time
//! in America/New_York.

use d3::time::date::{Date, LocalZone};
use serde_json::Value;
use std::{env, sync::Once};

/// The fixture's local zone. `chrono::Local` reads `TZ` from the
/// environment; nextest runs each test in a process of its own, so no other
/// thread reads the environment while this writes it.
fn new_york() -> LocalZone {
    static TZ: Once = Once::new();
    // SAFETY: the test's own thread is the only one, and sets `TZ` before
    // anything reads it.
    TZ.call_once(|| unsafe { env::set_var("TZ", "America/New_York") });
    LocalZone::default()
}

fn cases(key: &str) -> Vec<Vec<Value>> {
    let all: Value = serde_json::from_str(include_str!("d3/time_format.json")).expect("fixture");
    serde_json::from_value(all[key].clone()).expect("cases")
}

fn text(value: &Value) -> &str {
    value.as_str().expect("text")
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
fn formats() {
    let zone = new_york();
    let utc_zone = LocalZone::utc();
    let cases = cases("time");
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| {
            let time = case[1].as_f64().unwrap_or(f64::NAN);
            let local = zone.date(time).format(text(&case[0]));
            let utc = utc_zone.date(time).format(text(&case[0]));
            (local != text(&case[2]) || utc != text(&case[3]))
                .then(|| format!("{case:?}: {local:?} {utc:?}"))
        })
        .collect();
    assert_none(failures, cases.len());
}

#[test]
fn parses() {
    let zone = new_york();
    let utc_zone = LocalZone::utc();
    let cases = cases("parse");
    // `null`, an invalid date as "NaN", or a time value.
    let read = |date: Option<Date>| match date {
        None => Value::Null,
        Some(date) if !date.is_valid() => Value::from("NaN"),
        Some(date) => Value::from(date.time()),
    };
    let same = |got: &Value, want: &Value| {
        got.as_f64()
            .zip(want.as_f64())
            .map_or(got == want, |(a, b)| a == b)
    };
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| {
            let (specifier, input) = (text(&case[0]), text(&case[1]));
            let local = read(zone.time_parse(specifier, input));
            let utc = read(utc_zone.time_parse(specifier, input));
            (!same(&local, &case[2]) || !same(&utc, &case[3]))
                .then(|| format!("{case:?}: {local} {utc}"))
        })
        .collect();
    assert_none(failures, cases.len());
}

/// vega-format's multi-format, against what Vega 6.4.0 writes.
#[test]
fn multi_formats_are_vegas() {
    let zone = LocalZone::utc();
    let written: Vec<String> = [
        "2024-01-01T00:00Z",
        "2024-03-01T00:00Z",
        "2024-03-03T00:00Z",
        "2024-03-05T00:00Z",
        "2024-03-05T10:00Z",
        "2024-03-05T10:30Z",
        "2024-03-05T10:30:15Z",
        "2024-03-05T10:30:15.250Z",
    ]
    .iter()
    .map(|text| zone.parse(text).multi_format())
    .collect();
    assert_eq!(
        written,
        [
            "2024", "March", "Mar 03", "Tue 05", "10 AM", "10:30", ":15", ".250"
        ]
    );
}
