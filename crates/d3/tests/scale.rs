//! d3-array's ticks, d3-scale's scales, d3-interpolate and vega-scale's band
//! scales and schemes for every case in `d3/scale.json`, which
//! `d3/fixtures.mjs` writes from the pinned packages with local time in
//! America/New_York.

use d3::{
    array,
    color::Rgb,
    format::number::Number,
    interpolate::Interpolate,
    scale::{
        Band, BinOrdinal, Continuous, Diverging, Quantile, Quantize, Sequential, Threshold,
        Transform,
    },
    scheme::Scheme,
    time::{Interval, Unit, date::LocalZone},
};
use serde_json::Value;
use std::{env, sync::Once};

fn fixture() -> Value {
    serde_json::from_str(include_str!("d3/scale.json")).expect("fixture")
}

/// The fixture's local zone, as `time_format.rs` sets it.
fn new_york() -> LocalZone {
    static TZ: Once = Once::new();
    // SAFETY: the test's own thread is the only one, and sets `TZ` before
    // anything reads it.
    TZ.call_once(|| unsafe { env::set_var("TZ", "America/New_York") });
    LocalZone::default()
}

/// A number, or `NaN` and the infinities written as strings.
fn figure(value: &Value) -> f64 {
    match value {
        Value::Number(n) => n.as_f64().expect("number"),
        Value::String(text) => Number::from(text.as_str()).0,
        other => panic!("not a number: {other}"),
    }
}

fn figures(value: &Value) -> Vec<f64> {
    value.as_array().expect("list").iter().map(figure).collect()
}

/// Equal, `NaN` to `NaN`, or within what one libm's last bit makes of it.
fn same(a: f64, b: f64) -> bool {
    a == b || (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-12 * a.abs().max(b.abs())
}

fn all_same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(*a, *b))
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
fn ticks_match_d3_array() {
    let all = fixture();
    let cases = all["ticks"].as_array().expect("cases");
    let mut failures = Vec::new();
    for case in cases {
        let (start, stop, count) = (figure(&case[0]), figure(&case[1]), figure(&case[2]));
        let ticks = array::ticks(start, stop, count);
        let increment = array::tick_increment(start, stop, count);
        let step = array::tick_step(start, stop, count);
        let (lo, hi) = array::nice(start, stop, count);
        if !all_same(&ticks, &figures(&case[3]))
            || !same(increment, figure(&case[4]))
            || !same(step, figure(&case[5]))
            || !all_same(&[lo, hi], &figures(&case[6]))
        {
            failures.push(format!(
                "{case}\n  {ticks:?} {increment} {step} {:?}",
                [lo, hi]
            ));
        }
    }
    assert_none(failures, cases.len());
}

fn transform(kind: &str, parameter: &Value) -> Transform {
    let parameter = parameter.as_f64().unwrap_or(f64::NAN);
    match kind {
        "linear" => Transform::Linear,
        "log" => Transform::Log { base: parameter },
        "pow" => Transform::Pow {
            exponent: parameter,
        },
        "sqrt" => Transform::Pow { exponent: 0.5 },
        "symlog" => Transform::Symlog {
            constant: parameter,
        },
        other => panic!("no scale {other}"),
    }
}

#[test]
fn continuous_scales_match_d3() {
    let all = fixture();
    let cases = all["continuous"].as_array().expect("cases");
    let mut failures = Vec::new();
    for case in cases {
        let scale = Continuous {
            clamp: case["clamp"] == true,
            round: case["round"] == true,
            ..Continuous::new(
                transform(case["kind"].as_str().unwrap(), &case["parameter"]),
                figures(&case["domain"]),
                figures(&case["range"]),
            )
        };
        let pairs = |key: &str| -> Vec<(f64, Value)> {
            case[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| (figure(&pair[0]), pair[1].clone()))
                .collect()
        };
        for (x, y) in pairs("at") {
            let at = scale.at(x);
            if !same(at, figure(&y)) {
                failures.push(format!("{case:.80}\n  at {x}: {at}, d3 {y}"));
            }
        }
        for (y, x) in pairs("invert") {
            let invert = scale.invert(y);
            if !same(invert, figure(&x)) {
                failures.push(format!(
                    "{}\n  invert {y}: {invert}, d3 {x}",
                    case["domain"]
                ));
            }
        }
        for (count, ticks) in pairs("ticks") {
            let got = scale.ticks(count);
            if !all_same(&got, &figures(&ticks)) {
                failures.push(format!(
                    "{} {}\n  ticks {count}: {got:?}, d3 {ticks}",
                    case["kind"], case["domain"]
                ));
            }
        }
        for (count, domain) in pairs("nice") {
            let mut niced = scale.clone();
            niced.nice(count);
            if !all_same(&niced.domain, &figures(&domain)) {
                failures.push(format!(
                    "{} {}\n  nice {count}: {:?}, d3 {domain}",
                    case["kind"], case["domain"], niced.domain
                ));
            }
        }
    }
    assert_none(failures, cases.len());
}

#[test]
fn time_scales_match_d3() {
    let all = fixture();
    let cases = all["time"].as_array().expect("cases");
    let local = new_york();
    let mut failures = Vec::new();
    for case in cases {
        let zone = match case["utc"] == true {
            true => LocalZone::utc(),
            false => local.clone(),
        };
        let scale = Continuous::new(
            Transform::Time(zone),
            figures(&case["domain"]),
            vec![0.0, 1.0],
        );
        for pair in case["ticks"].as_array().unwrap() {
            let got = scale.ticks(figure(&pair[0]));
            if !all_same(&got, &figures(&pair[1])) {
                failures.push(format!(
                    "{} {}\n  ticks {}: {got:?}",
                    case["utc"], case["domain"], pair[0]
                ));
            }
        }
        for pair in case["nice"].as_array().unwrap() {
            let mut niced = scale.clone();
            niced.nice(figure(&pair[0]));
            if !all_same(&niced.domain, &figures(&pair[1])) {
                failures.push(format!(
                    "{} {}\n  nice {}: {:?}, d3 {}",
                    case["utc"], case["domain"], pair[0], niced.domain, pair[1]
                ));
            }
        }
        for every in case["every"].as_array().unwrap() {
            let unit = match every[0].as_str().unwrap() {
                "minute" => Unit::Minute,
                "hour" => Unit::Hour,
                "day" => Unit::Day,
                "week" => Unit::Week,
                "month" => Unit::Month,
                _ => Unit::Year,
            };
            let mut niced = scale.clone();
            let Transform::Time(zone) = &scale.transform else {
                unreachable!()
            };
            niced.nice_to(&Interval::every(unit, figure(&every[1]), 0.0, zone));
            if !all_same(&niced.domain, &figures(&every[2])) {
                failures.push(format!(
                    "{} {}\n  nice every {}: {:?}",
                    case["utc"], case["domain"], every, niced.domain
                ));
            }
        }
    }
    assert_none(failures, cases.len());
}

#[test]
fn band_and_point_scales_match_vega() {
    let all = fixture();
    let cases = all["band"].as_array().expect("cases");
    let mut failures = Vec::new();
    for case in cases {
        let range = figures(&case["range"]);
        let band = Band {
            count: case["count"].as_u64().unwrap() as usize,
            range: [range[0], range[1]],
            padding_inner: figure(&case["inner"]),
            padding_outer: figure(&case["outer"]),
            align: figure(&case["align"]),
            round: case["round"] == true,
        };
        let at: Vec<f64> = (0..band.count).map(|i| band.at(i)).collect();
        let inverted: Vec<Value> = case["invert"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| match band.invert(figure(&pair[0])) {
                Some(index) => Value::from(index),
                None => Value::Null,
            })
            .collect();
        let expected: Vec<Value> = case["invert"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| pair[1].clone())
            .collect();
        if !same(band.step(), figure(&case["step"]))
            || !same(band.bandwidth(), figure(&case["bandwidth"]))
            || !all_same(&at, &figures(&case["at"]))
            || inverted != expected
        {
            failures.push(format!(
                "{case}\n  {} {} {at:?} {inverted:?}",
                band.step(),
                band.bandwidth()
            ));
        }
    }
    assert_none(failures, cases.len());
}

#[test]
fn discretizing_scales_match_d3() {
    let all = fixture();
    let cases = all["discretizing"].as_array().expect("cases");
    let mut failures = Vec::new();
    for case in cases {
        let count = case["count"].as_u64().unwrap() as usize;
        let index: Box<dyn Fn(f64) -> Option<usize>> = match case["kind"].as_str().unwrap() {
            "quantile" => {
                let scale = Quantile::new(&figures(&case["values"]), count);
                Box::new(move |x| scale.index(x))
            }
            "quantize" => {
                let domain = figures(&case["domain"]);
                let scale = Quantize::new([domain[0], domain[1]], count);
                Box::new(move |x| scale.index(x))
            }
            "threshold" => {
                let scale = Threshold {
                    domain: figures(&case["domain"]),
                    count,
                };
                Box::new(move |x| scale.index(x))
            }
            _ => {
                let scale = BinOrdinal {
                    bins: figures(&case["bins"]),
                    count,
                };
                Box::new(move |x| scale.index(x))
            }
        };
        for pair in case["at"].as_array().unwrap() {
            let got = index(figure(&pair[0]))
                .map(Value::from)
                .unwrap_or(Value::Null);
            if got != pair[1] {
                failures.push(format!("{case:.120}\n  {} {got}", pair[0]));
            }
        }
    }
    assert_none(failures, cases.len());
}

#[test]
fn sequential_and_diverging_scales_match_d3() {
    let all = fixture();
    let cases = all["sequential"].as_array().expect("cases");
    let mut failures = Vec::new();
    for case in cases {
        let transform = transform(case["kind"].as_str().unwrap(), &case["parameter"]);
        let domain = figures(&case["domain"]);
        let clamp = case["clamp"] == true;
        let at: Box<dyn Fn(f64) -> f64> = match domain[..] {
            [a, b] => {
                let scale = Sequential {
                    transform,
                    domain: [a, b],
                    clamp,
                };
                Box::new(move |x| scale.at(x))
            }
            [a, b, c] => {
                let scale = Diverging {
                    transform,
                    domain: [a, b, c],
                    clamp,
                };
                Box::new(move |x| scale.at(x))
            }
            _ => panic!("domain"),
        };
        for pair in case["at"].as_array().unwrap() {
            let got = at(figure(&pair[0]));
            if !same(got, figure(&pair[1])) {
                failures.push(format!(
                    "{} {:?} {clamp}\n  {}: {got}, d3 {}",
                    case["kind"], domain, pair[0], pair[1]
                ));
            }
        }
    }
    assert_none(failures, cases.len());
}

fn rgb(text: &str) -> Rgb {
    text.parse().expect("colour")
}

#[test]
fn colour_interpolators_match_d3() {
    let all = fixture();
    let cases = all["between"].as_array().expect("cases");
    let ts = [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0, -0.2, 1.3];
    let mut failures = Vec::new();
    for case in cases {
        let method: Interpolate = case[0].as_str().unwrap().parse().expect("method");
        let method = method.with_gamma(figure(&case[1]));
        let expected = case[4].as_array().unwrap();
        for (t, expected) in ts.iter().zip(expected) {
            let got = match case[2].as_str().unwrap() {
                "piecewise" => {
                    method.piecewise(&["#440154", "white", "#21918c", "orange"].map(rgb), *t)
                }
                a => method.color(rgb(a), rgb(case[3].as_str().unwrap()), *t),
            };
            if got.to_string() != expected.as_str().unwrap() {
                failures.push(format!("{case:.100}\n  {t}: {got}, d3 {expected}"));
            }
        }
    }
    assert_none(failures, cases.len());
}

#[test]
fn schemes_match_vega() {
    let all = fixture();
    let cases = all["schemes"].as_array().expect("cases");
    let ts = [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0, -0.2, 1.3];
    let mut failures = Vec::new();
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let scheme: Scheme = name.parse().expect("scheme");
        if let Some(colors) = case["colors"].as_array() {
            let got: Vec<String> = scheme
                .colors()
                .iter()
                .map(|c| format!("#{:02x}{:02x}{:02x}", c.r as u8, c.g as u8, c.b as u8))
                .collect();
            let expected: Vec<&str> = colors.iter().map(|c| c.as_str().unwrap()).collect();
            if scheme.is_continuous() || got != expected {
                failures.push(format!("{name}: {got:?}"));
            }
            continue;
        }
        for sample in case["samples"].as_array().unwrap() {
            let extent = sample[0].as_array().map(|e| [figure(&e[0]), figure(&e[1])]);
            let reverse = sample[1] == true;
            for (t, expected) in ts.iter().zip(sample[2].as_array().unwrap()) {
                let got = scheme.at(*t, extent, reverse).to_string();
                if got != expected.as_str().unwrap() {
                    failures.push(format!(
                        "{name} {extent:?} {reverse} {t}: {got}, vega {expected}"
                    ));
                }
            }
        }
        for quantized in case["quantized"].as_array().unwrap() {
            let count = quantized[0].as_u64().unwrap() as usize;
            for (extent, expected) in [(None, &quantized[1]), (Some([0.2, 0.8]), &quantized[2])] {
                let got: Vec<String> = scheme
                    .discrete(count, extent, false)
                    .iter()
                    .map(Rgb::to_string)
                    .collect();
                let expected: Vec<&str> = expected
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|c| c.as_str().unwrap())
                    .collect();
                if got != expected {
                    failures.push(format!(
                        "{name} {count} {extent:?}: {got:?}, vega {expected:?}"
                    ));
                }
            }
        }
    }
    assert_none(failures, cases.len());
}
