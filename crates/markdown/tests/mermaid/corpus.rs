//! Every corpus entry draws: Mermaid's documentation examples, by kind.

use std::{fs, path::Path};

use markdown::mermaid::{Options, render};

fn draws(file: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/mermaid/corpus/{file}"));
    let sources: Vec<String> =
        serde_json::from_str(&fs::read_to_string(&path).expect("corpus file"))
            .expect("corpus JSON");
    for (i, source) in sources.iter().enumerate() {
        let drawing = render(source, &Options::default())
            .unwrap_or_else(|error| panic!("{file} #{i}: {error}"));
        assert!(drawing.svg.starts_with("<svg"), "{file} #{i}");
        assert!(drawing.width > 0.0 && drawing.height > 0.0, "{file} #{i}");
    }
}

#[test]
fn info() {
    draws("info-docs.json");
}

#[test]
fn pie() {
    draws("pie-docs.json");
}

#[test]
fn radar() {
    draws("radar-docs.json");
}

#[test]
fn treemap() {
    draws("treemap-docs.json");
}

#[test]
fn sankey() {
    draws("sankey-docs.json");
}

#[test]
fn quadrant() {
    draws("quadrant-docs.json");
}

#[test]
fn xychart() {
    draws("xychart-docs.json");
}

#[test]
fn packet() {
    draws("packet-docs.json");
}

#[test]
fn treeview() {
    draws("treeview-docs.json");
}

#[test]
fn git() {
    draws("git-docs.json");
}

#[test]
fn mindmap() {
    draws("mindmap-docs.json");
}

#[test]
fn railroad() {
    draws("railroad-docs.json");
}

#[test]
fn venn() {
    draws("venn-docs.json");
}

#[test]
fn ishikawa() {
    draws("ishikawa-docs.json");
}

#[test]
fn journey() {
    draws("journey-docs.json");
}

#[test]
fn kanban() {
    draws("kanban-docs.json");
}

#[test]
fn gantt() {
    draws("gantt-docs.json");
}

#[test]
fn timeline() {
    draws("timeline-docs.json");
}

#[test]
fn class() {
    draws("class-docs.json");
}
