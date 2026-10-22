//! Each gallery spec compiled, against `vegaLite.compile(spec).spec`:
//! `VEGA_REFERENCE=<dir>` names the directory `reference/reference.mjs`
//! writes. The specs that compile to exactly that, numbers as `f64`, are
//! those named in `compile.txt`.

use serde_json::Value as Json;
use std::{collections::BTreeSet, env, fs, path::Path};
use vega_lite::spec::TopLevel;

/// The first place `ours` and `theirs` differ.
fn diff(path: &str, ours: &Json, theirs: &Json) -> Option<String> {
    match (ours, theirs) {
        (Json::Number(a), Json::Number(b)) => {
            (a.as_f64() != b.as_f64()).then(|| format!("{path}: {a} against {b}"))
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

fn compiled(specs: &Path, name: &str) -> Result<Json, String> {
    let text =
        fs::read_to_string(specs.join(format!("{name}.vl.json"))).map_err(|e| e.to_string())?;
    let spec: TopLevel = serde_json::from_str(&text).map_err(|e| format!("parse: {e}"))?;
    let vega = vega_lite::compile(&spec).map_err(|e| e.to_string())?;
    serde_json::to_value(vega).map_err(|e| e.to_string())
}

#[test]
#[ignore = "needs VEGA_REFERENCE"]
fn the_specs_that_compile_exactly_are_the_pass_list() {
    let Ok(reference) = env::var("VEGA_REFERENCE") else {
        panic!("VEGA_REFERENCE=<dir> written by reference.mjs");
    };
    let reference = Path::new(&reference);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let specs = root.join("tests/corpus/specs");
    let listed: BTreeSet<String> = fs::read_to_string(root.join("tests/compile.txt"))
        .expect("pass list")
        .lines()
        .map(str::to_owned)
        .collect();
    let only = env::var("VEGA_SPEC").ok();
    let mut names: Vec<String> = fs::read_dir(&specs)
        .expect("specs")
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            Some(name.strip_suffix(".vl.json")?.to_owned())
        })
        .collect();
    names.sort();
    let mut passing = BTreeSet::new();
    for name in names {
        if only.as_ref().is_some_and(|only| *only != name) {
            continue;
        }
        let Ok(text) = fs::read_to_string(reference.join(format!("{name}.vg.json"))) else {
            continue;
        };
        let theirs: Json = serde_json::from_str(&text).expect("compiled JSON");
        match compiled(&specs, &name) {
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
        "now compile, add to compile.txt: {gained:?}\nno longer compile: {lost:?}"
    );
}
