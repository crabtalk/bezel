//! d3-shape's paths for every case in `d3/shape.json`, which
//! `d3/fixtures.mjs` writes from the pinned d3-shape.

use d3::{
    format::{number, number::Number},
    shape::{arc::Sector, curve::Curve, path::Segment},
};
use serde_json::Value;

fn curve(name: &str, parameter: f64) -> Curve {
    match name {
        "Linear" => Curve::Linear,
        "LinearClosed" => Curve::LinearClosed,
        "Step" => Curve::Step,
        "StepBefore" => Curve::StepBefore,
        "StepAfter" => Curve::StepAfter,
        "Basis" => Curve::Basis,
        "BasisOpen" => Curve::BasisOpen,
        "BasisClosed" => Curve::BasisClosed,
        "Bundle" => Curve::Bundle(parameter),
        "Cardinal" => Curve::Cardinal(parameter),
        "CardinalOpen" => Curve::CardinalOpen(parameter),
        "CardinalClosed" => Curve::CardinalClosed(parameter),
        "CatmullRom" => Curve::CatmullRom(parameter),
        "CatmullRomOpen" => Curve::CatmullRomOpen(parameter),
        "CatmullRomClosed" => Curve::CatmullRomClosed(parameter),
        "MonotoneX" => Curve::MonotoneX,
        "MonotoneY" => Curve::MonotoneY,
        "Natural" => Curve::Natural,
        other => panic!("no curve {other}"),
    }
}

/// `segments` as d3-shape writes them, to d3-path's three digits.
fn path(segments: &[Segment]) -> String {
    let n = |v: f64| Number(number::round(v * 1000.0) / 1000.0).to_string();
    let p = |[x, y]: [f64; 2]| format!("{},{}", n(x), n(y));
    segments
        .iter()
        .map(|segment| match *segment {
            Segment::Move(to) => format!("M{}", p(to)),
            Segment::Line(to) => format!("L{}", p(to)),
            Segment::Cubic(a, b, to) => format!("C{},{},{}", p(a), p(b), p(to)),
            Segment::Arc {
                radius,
                large,
                sweep,
                to,
            } => format!(
                "A{},{},0,{},{},{}",
                n(radius),
                n(radius),
                u8::from(large),
                u8::from(sweep),
                p(to)
            ),
            Segment::Close => "Z".to_string(),
        })
        .collect()
}

#[test]
fn lines_and_areas_match_d3() {
    let all: Value = serde_json::from_str(include_str!("d3/shape.json")).expect("fixture");
    let cases = all["curve"].as_array().expect("cases");
    let mut failures = Vec::new();
    for case in cases {
        let parameter = case[1].as_f64().unwrap_or(f64::NAN);
        let curve = curve(case[0].as_str().expect("name"), parameter);
        let run: Vec<[f64; 2]> = serde_json::from_value(case[2].clone()).expect("run");
        let line = path(&curve.line(&run));
        // d3 answers `null` for an empty path.
        if line != case[3].as_str().unwrap_or_default() {
            failures.push(format!("{case}\n  line {line}"));
        }
        // Bundle draws lines alone.
        if case[0] != "Bundle" {
            let expected = case[4].as_str().unwrap_or_default();
            let base: Vec<[f64; 2]> = run.iter().map(|&[x, y]| [x, y - 7.0]).collect();
            let area = path(&curve.area(&run, &base));
            if area != expected {
                failures.push(format!("{case}\n  area {area}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

#[test]
fn sectors_match_d3() {
    let all: Value = serde_json::from_str(include_str!("d3/shape.json")).expect("fixture");
    let cases = all["arc"].as_array().expect("cases");
    let mut failures = Vec::new();
    for case in cases {
        let at = |index: usize| case[index].as_f64().expect("number");
        let sector = Sector {
            inner: at(0),
            outer: at(1),
            start: at(2),
            end: at(3),
            pad: at(4),
            corner: at(5),
        };
        let written = path(&sector.path());
        if written != case[6].as_str().expect("path") {
            failures.push(format!("{case}\n  arc {written}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}
