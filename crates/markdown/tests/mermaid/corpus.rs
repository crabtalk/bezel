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
