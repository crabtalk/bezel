//! Each Vega spec Vega-Lite compiles the gallery to, run, against the
//! scenegraph Vega draws from it: `VEGA_REFERENCE=<dir>` names the
//! directory `bezel-vega-lite`'s `tests/reference/reference.mjs` writes.
//! The specs whose scenegraphs match, numbers to a hundredth, and whose views
//! take the size, origin and padding Vega gives them, are exactly those
//! named in `scenegraph.txt`.

use serde_json::Value as Json;
use std::{collections::BTreeSet, env, fs, path::Path};
use vega::{View, spec::Spec};

const JSDELIVR: &str = "https://cdn.jsdelivr.net/npm/vega-datasets@";

/// The first place `ours` and `theirs` differ, numbers to a hundredth.
fn diff(path: &str, ours: &Json, theirs: &Json) -> Option<String> {
    match (ours, theirs) {
        (Json::Number(a), Json::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            ((a - b).abs() > 0.005).then(|| format!("{path}: {a} against {b}"))
        }
        (Json::Object(a), Json::Object(b)) => {
            for key in a.keys().chain(b.keys()) {
                match (a.get(key), b.get(key)) {
                    (Some(x), Some(y)) => {
                        if let Some(d) = diff(&format!("{path}.{key}"), x, y) {
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
                .find_map(|(i, (x, y))| diff(&format!("{path}[{i}]"), x, y))
        }
        (a, b) => (a != b).then(|| format!("{path}: {a} against {b}")),
    }
}

/// Our scenegraph and view placement for the reference's compiled spec `name`, or why there is
/// none.
fn scenegraph(reference: &Path, data: &Path, name: &str) -> Result<Json, String> {
    let text =
        fs::read_to_string(reference.join(format!("{name}.vg.json"))).map_err(|e| e.to_string())?;
    let spec: Spec = serde_json::from_str(&text).map_err(|e| format!("parse: {e}"))?;
    let load = |url: &str| {
        let path = match url.strip_prefix(JSDELIVR) {
            Some(rest) => rest.split_once('/')?.1,
            None => url,
        };
        fs::read_to_string(data.join(path.strip_prefix("data/")?)).ok()
    };
    let view = View::new(spec, &load).map_err(|e| e.to_string())?;
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

#[test]
#[ignore = "needs VEGA_REFERENCE"]
fn the_scenegraphs_that_match_are_the_pass_list() {
    let Ok(reference) = env::var("VEGA_REFERENCE") else {
        panic!("VEGA_REFERENCE=<dir> written by reference.mjs");
    };
    let reference = Path::new(&reference);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = root.join("../vega-lite/tests/corpus/data");
    let listed: BTreeSet<String> = fs::read_to_string(root.join("tests/scenegraph.txt"))
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
            Some(name.strip_suffix(".sg.json")?.to_owned())
        })
        .collect();
    names.sort();
    for name in names {
        if only.as_ref().is_some_and(|only| *only != name) {
            continue;
        }
        let read = |suffix: &str| -> Json {
            serde_json::from_str(
                &fs::read_to_string(reference.join(format!("{name}.{suffix}.json")))
                    .expect("reference"),
            )
            .expect("reference JSON")
        };
        let theirs = serde_json::json!({"scene": read("sg"), "view": read("view")});
        match scenegraph(reference, &data, &name) {
            Ok(ours) => match diff("", &ours, &theirs) {
                None => {
                    passing.insert(name);
                }
                Some(d) if only.is_some() => println!("{name}: {d}"),
                Some(_) => {}
            },
            Err(e) if only.is_some() => println!("{name}: {e}"),
            Err(_) => {}
        }
    }
    if only.is_some() {
        return;
    }
    let gained: Vec<_> = passing.difference(&listed).collect();
    let lost: Vec<_> = listed.difference(&passing).collect();
    assert!(
        gained.is_empty() && lost.is_empty(),
        "now match, add to scenegraph.txt: {gained:?}\nno longer match: {lost:?}"
    );
}
