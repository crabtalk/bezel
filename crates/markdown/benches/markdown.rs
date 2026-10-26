//! This crate timed on the cases `tests/tiptap/bench.mjs` times tiptap on,
//! with the same timing loop, written as JSON for `bench/compare.mjs`:
//!
//! ```sh
//! cargo bench -p bezel-markdown --bench markdown > ours.json
//! ```

use std::{fs, path::Path, time::Instant};

use markdown::{Doc, html, parse, serialize};

#[path = "../tests/spec/examples.rs"]
mod examples;

/// One warm-up pass, then passes until 300 ms are spent and at least 5 are
/// taken (at most 2000): the median pass in ms.
fn time<T, R>(inputs: &[T], stage: impl Fn(&T) -> R) -> f64 {
    let run = || {
        for input in inputs {
            std::hint::black_box(stage(input));
        }
    };
    run();
    let mut budget = 300.0;
    let mut samples = Vec::new();
    while budget > 0.0 || samples.len() < 5 {
        let start = Instant::now();
        run();
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        samples.push(ms);
        budget -= ms;
        if samples.len() >= 2000 {
            break;
        }
    }
    samples.sort_by(f64::total_cmp);
    samples[samples.len() >> 1]
}

fn main() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = here.join("../..");
    let read = |path: &Path| fs::read_to_string(path).expect("a markdown file");

    // The workspace's own READMEs, in the order the tiptap bench reads them.
    let mut docs = vec![
        read(&root.join("README.md")),
        read(&root.join("CONTRIBUTING.md")),
    ];
    let mut crates: Vec<_> = fs::read_dir(root.join("crates"))
        .expect("crates")
        .filter_map(|entry| Some(entry.ok()?.path().join("README.md")))
        .filter(|readme| readme.exists())
        .collect();
    crates.sort();
    docs.extend(crates.iter().map(|readme| read(readme)));
    let spec = fs::read_to_string(here.join("tests/spec/commonmark.txt"))
        .expect("tests/spec/commonmark.txt: run crates/markdown/tests/spec/fetch.sh");
    let examples = examples::examples(&spec, &[""])
        .into_iter()
        .map(|example| example.markdown)
        .collect();
    let cases = [
        ("workspace READMEs", docs),
        ("CommonMark spec examples", examples),
        ("CommonMark spec", vec![spec]),
    ];

    let mut rows = Vec::new();
    for (name, sources) in &cases {
        let docs: Vec<Doc> = sources.iter().map(|source| parse(source)).collect();
        let stages = [
            ("parse", time(sources, |source| parse(source))),
            ("html", time(&docs, html)),
            ("serialize", time(&docs, serialize)),
        ];
        for (stage, ms) in stages {
            rows.push(serde_json::json!({
                "case": format!("{stage}: {name}"),
                "inputs": sources.len(),
                "ms": ms,
                "done": sources.len(),
            }));
        }
    }
    let engine = format!("bezel-markdown {}", env!("CARGO_PKG_VERSION"));
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({ "engine": engine, "rows": rows }))
            .expect("JSON")
    );
}
