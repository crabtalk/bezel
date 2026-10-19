//! Draws every `<name>.mmd` in a directory with `bezel-mermaid` and prints
//! `{ "<name>": "<svg>" }` for the website to inline.
//!
//! The SVG keeps its `--bezel-*` variables; the page sets them.

use std::{collections::BTreeMap, path::Path};

fn main() {
    let dir = std::env::args().nth(1).expect("usage: diagrams <dir>");
    let mut drawn = BTreeMap::new();
    for entry in std::fs::read_dir(&dir).expect("diagrams directory") {
        let path = entry.expect("directory entry").path();
        if path.extension().is_none_or(|ext| ext != "mmd") {
            continue;
        }
        let name = stem(&path);
        let source = std::fs::read_to_string(&path).expect("diagram source");
        let options = mermaid::Options {
            id_prefix: format!("home-{name}"),
            ..Default::default()
        };
        let drawing = mermaid::render(&source, &options, &mermaid::Metrics)
            .unwrap_or_else(|error| panic!("{} does not draw: {error:?}", path.display()));
        drawn.insert(name, drawing.svg);
    }
    println!("{}", serde_json::to_string(&drawn).expect("json"));
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .expect("utf-8 file name")
        .to_string()
}
