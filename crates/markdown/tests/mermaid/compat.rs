//! Every `compat/<kind>.json` holds sources and what Pele's `parse` made of
//! them: a model, or an error. Written by `pele/render.mjs model`.

use std::{fs, path::Path};

use markdown::mermaid::{Options, parse};
use serde_json::Value;

fn cases(name: &str) -> Vec<Value> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/mermaid/compat/{name}.json"));
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).expect("compat cases are JSON")
}

fn check(name: &str) {
    let mut failures = Vec::new();
    for case in cases(name) {
        let source = case["source"].as_str().expect("a source");
        let ours = match parse(source, &Options::default()) {
            Ok(parsed) => {
                serde_json::json!({ "model": serde_json::to_value(&parsed.model).unwrap() })
            }
            Err(error) => serde_json::json!({
                "error": { "code": serde_json::to_value(error.code).unwrap(), "message": error.message }
            }),
        };
        let theirs = match case.get("model") {
            Some(model) => serde_json::json!({ "model": model }),
            None => serde_json::json!({ "error": case["error"] }),
        };
        if ours != theirs {
            failures.push(format!(
                "source {source:?}\n  pele: {theirs}\n  ours: {ours}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {name}:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn info() {
    check("info");
}

#[test]
fn pie() {
    check("pie");
}

#[test]
fn radar() {
    check("radar");
}

#[test]
fn treemap() {
    check("treemap");
}

#[test]
fn sankey() {
    check("sankey");
}

#[test]
fn quadrant() {
    check("quadrant");
}

#[test]
fn xychart() {
    check("xychart");
}

#[test]
fn packet() {
    check("packet");
}

#[test]
fn treeview() {
    check("treeview");
}

#[test]
fn git() {
    check("git");
}

#[test]
fn mindmap() {
    check("mindmap");
}

#[test]
fn railroad() {
    check("railroad");
}

#[test]
fn venn() {
    check("venn");
}

#[test]
fn ishikawa() {
    check("ishikawa");
}
