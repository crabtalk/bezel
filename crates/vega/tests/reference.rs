//! Each Vega spec Vega-Lite compiles the gallery to, run, against what Vega
//! makes of it: `VEGA_REFERENCE=<dir>` names the directory
//! `bezel-vega-lite`'s `tests/reference/reference.mjs` writes.
//!
//! The specs whose scenegraphs match, numbers to a hundredth, and whose views
//! take the size, origin and padding Vega gives them, are exactly those
//! named in `scenegraph.txt`. The specs each of whose top-level data sets
//! holds the tuples Vega's does, numbers as `f64`, are exactly those named in
//! `data.txt`.
//!
//! Times are read in UTC, as the reference reads them. Each spec that does
//! not match prints where it differs; `--no-capture`
//! shows them. `VEGA_SPEC=<name>` checks that spec alone.

use serde_json::Value as Json;
use std::{collections::BTreeSet, env, fs, path::Path};
use vega::{View, spec::Spec};

const JSDELIVR: &str = "https://cdn.jsdelivr.net/npm/vega-datasets@";

/// The first place `ours` and `theirs` differ, numbers within `tolerance`.
fn diff(path: &str, ours: &Json, theirs: &Json, tolerance: f64) -> Option<String> {
    match (ours, theirs) {
        (Json::Number(a), Json::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            ((a - b).abs() > tolerance).then(|| format!("{path}: {a} against {b}"))
        }
        (Json::Object(a), Json::Object(b)) => {
            for key in a.keys().chain(b.keys()) {
                match (a.get(key), b.get(key)) {
                    (Some(x), Some(y)) => {
                        if let Some(d) = diff(&format!("{path}.{key}"), x, y, tolerance) {
                            return Some(d);
                        }
                    }
                    (x, y) => return Some(format!("{path}.{key}: {x:?} against {y:?}")),
                }
            }
            None
        }
        (Json::Array(a), Json::Array(b)) => {
            if a.len() != b.len() {
                return Some(format!("{path}: {} items against {}", a.len(), b.len()));
            }
            a.iter()
                .zip(b)
                .enumerate()
                .find_map(|(i, (x, y))| diff(&format!("{path}[{i}]"), x, y, tolerance))
        }
        (a, b) => (a != b).then(|| format!("{path}: {a} against {b}")),
    }
}

/// The view of `part` of the reference's compiled spec `name`, its urls read
/// from the corpus data.
fn run<T>(
    reference: &Path,
    name: &str,
    part: fn(Json) -> Json,
    then: impl FnOnce(&View) -> Result<T, String>,
) -> Result<T, String> {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../vega-lite/tests/corpus/data");
    let text =
        fs::read_to_string(reference.join(format!("{name}.vg.json"))).map_err(|e| e.to_string())?;
    let json: Json = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let spec: Spec = serde_json::from_value(part(json)).map_err(|e| format!("parse: {e}"))?;
    let load = |url: &str| {
        let path = match url.strip_prefix(JSDELIVR) {
            Some(rest) => rest.split_once('/')?.1,
            None => url,
        };
        fs::read_to_string(data.join(path.strip_prefix("data/")?)).ok()
    };
    let view = View::new(spec, &load).map_err(|e| e.to_string())?;
    then(&view)
}

/// Our scenegraph and view placement.
fn scenegraph(view: &View) -> Result<Json, String> {
    let drawing = view.drawing().map_err(|e| e.to_string())?;
    let [top, right, bottom, left] = drawing.padding;
    Ok(serde_json::json!({
        "scene": drawing.scene.to_json(),
        "view": {
            "width": drawing.width,
            "height": drawing.height,
            "origin": drawing.origin,
            "padding": {"top": top, "bottom": bottom, "left": left, "right": right},
        },
    }))
}

/// The data side of a spec: its data sets, the view's size, and its
/// signals' values and updates. Vega's run fires no event, so a signal's
/// handlers and binding never change it.
fn data_side(mut spec: Json) -> Json {
    let spec = spec.as_object_mut().expect("spec");
    spec.retain(|key, _| {
        matches!(
            key.as_str(),
            "data" | "signals" | "width" | "height" | "padding" | "autosize"
        )
    });
    for signal in spec
        .get_mut("signals")
        .and_then(Json::as_array_mut)
        .into_iter()
        .flatten()
    {
        let signal = signal.as_object_mut().expect("signal");
        signal.retain(|key, _| matches!(key.as_str(), "name" | "value" | "update"));
    }
    Json::Object(std::mem::take(spec))
}

/// Our tuples of each top-level data set, as `JSON.stringify` writes them.
fn data(view: &View) -> Result<Json, String> {
    let mut sets = serde_json::Map::new();
    for name in view.data_names() {
        let tuples = view.data(name).map_err(|e| e.to_string())?;
        let tuples: Vec<Json> = tuples.iter().map(|t| Json::from(&t.datum)).collect();
        sets.insert(name.into(), Json::Array(tuples));
    }
    Ok(Json::Object(sets))
}

/// Checks each spec the reference has `<name>.<suffix>.json` for, `ours`
/// against `theirs`; the specs that match are exactly those in `list`.
fn check(
    list: &str,
    suffix: &str,
    tolerance: f64,
    part: fn(Json) -> Json,
    theirs: impl Fn(&Path, &str) -> Json,
    ours: impl Fn(&View) -> Result<Json, String>,
) {
    let Ok(reference) = env::var("VEGA_REFERENCE") else {
        panic!("VEGA_REFERENCE=<dir> written by reference.mjs");
    };
    // The reference runs in UTC. Each test is a process of its own.
    unsafe { env::set_var("TZ", "UTC") };
    let reference = Path::new(&reference);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let listed: BTreeSet<String> = fs::read_to_string(root.join("tests").join(list))
        .expect("pass list")
        .lines()
        .map(str::to_owned)
        .collect();
    let only = env::var("VEGA_SPEC").ok();
    let mut passing = BTreeSet::new();
    let mut names: Vec<String> = fs::read_dir(reference)
        .expect("reference")
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            Some(name.strip_suffix(&format!(".{suffix}.json"))?.to_owned())
        })
        .collect();
    names.sort();
    for name in names {
        if only.as_ref().is_some_and(|only| *only != name) {
            continue;
        }
        let theirs = theirs(reference, &name);
        match run(reference, &name, part, &ours) {
            Ok(ours) => match diff("", &ours, &theirs, tolerance) {
                None => {
                    passing.insert(name);
                }
                Some(d) => println!("{name}: {d}"),
            },
            Err(e) => println!("{name}: {e}"),
        }
    }
    if only.is_some() {
        return;
    }
    let gained: Vec<_> = passing.difference(&listed).collect();
    let lost: Vec<_> = listed.difference(&passing).collect();
    assert!(
        gained.is_empty() && lost.is_empty(),
        "now match, add to {list}: {gained:?}\nno longer match: {lost:?}"
    );
}

fn read(reference: &Path, name: &str, suffix: &str) -> Json {
    serde_json::from_str(
        &fs::read_to_string(reference.join(format!("{name}.{suffix}.json"))).expect("reference"),
    )
    .expect("reference JSON")
}

#[test]
#[ignore = "needs VEGA_REFERENCE"]
fn the_scenegraphs_that_match_are_the_pass_list() {
    check(
        "scenegraph.txt",
        "sg",
        0.005,
        |spec| spec,
        |reference, name| {
            serde_json::json!({
                "scene": read(reference, name, "sg"),
                "view": read(reference, name, "view"),
            })
        },
        scenegraph,
    );
}

#[test]
#[ignore = "needs VEGA_REFERENCE"]
fn the_data_that_match_are_the_pass_list() {
    check(
        "data.txt",
        "data",
        0.0,
        data_side,
        |reference, name| read(reference, name, "data"),
        data,
    );
}
