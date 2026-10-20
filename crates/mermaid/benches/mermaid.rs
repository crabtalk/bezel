//! The engine timed on the cases `tests/mermaid/pele/bench.mjs` times Pele
//! on, with Pele's timing loop, written as JSON for
//! `tests/mermaid/pele/compare.mjs`:
//!
//! ```sh
//! cargo bench -p bezel-mermaid --bench mermaid > ours.json
//! ```

use std::{fs, path::Path, time::Instant};

use mermaid::{Metrics, Options, render};

fn corpus(file: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/mermaid/corpus")
        .join(file);
    serde_json::from_str(&fs::read_to_string(path).expect("corpus file")).expect("corpus JSON")
}

fn chain(n: usize, fan: usize) -> String {
    let mut s = String::from("flowchart TD\n");
    for i in 1..n {
        let parent = (i - 1) / fan;
        s.push_str(&format!("  n{parent}[Node {parent}] --> n{i}[Node {i}]\n"));
    }
    s
}

/// Pele's generated mesh, with its generator: `seed * 1664525 + 1013904223`
/// in 32 bits.
fn mesh(n: usize, extra: usize) -> String {
    let mut s = String::from("flowchart TD\n");
    let mut seed: u32 = 7;
    let mut rnd = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(seed) / 4_294_967_296.0
    };
    for i in 1..n {
        let from = (rnd() * i as f64).floor() as usize;
        s.push_str(&format!("  n{from} --> n{i}\n"));
    }
    for i in 0..extra {
        let from = (rnd() * n as f64).floor() as usize;
        let to = (rnd() * n as f64).floor() as usize;
        s.push_str(&format!("  n{from} -->|label {i}| n{to}\n"));
    }
    s
}

/// One warm-up pass, then passes until 300 ms are spent and at least 5 are
/// taken (at most 2000): the median pass in ms, and how many drew.
fn time(inputs: &[String], options: &Options) -> (f64, usize) {
    let run = || {
        inputs
            .iter()
            .filter(|source| render(source, options, &Metrics).is_ok())
            .count()
    };
    let done = run();
    let mut budget = 300.0;
    let mut samples = Vec::new();
    while budget > 0.0 || samples.len() < 5 {
        let start = Instant::now();
        std::hint::black_box(run());
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        samples.push(ms);
        budget -= ms;
        if samples.len() >= 2000 {
            break;
        }
    }
    samples.sort_by(f64::total_cmp);
    (samples[samples.len() >> 1], done)
}

fn main() {
    let mut cases: Vec<(String, Vec<String>)> = vec![
        (
            "docs: all flowchart examples".into(),
            corpus("flowchart-docs.json"),
        ),
        ("tree: 50 nodes".into(), vec![chain(50, 3)]),
        (
            "mesh: 50 nodes, 20 extra labelled edges".into(),
            vec![mesh(50, 20)],
        ),
        ("tree: 500 nodes".into(), vec![chain(500, 3)]),
        (
            "mesh: 500 nodes, 200 extra labelled edges".into(),
            vec![mesh(500, 200)],
        ),
    ];
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mermaid/corpus");
    let mut files: Vec<String> = fs::read_dir(dir)
        .expect("corpus")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.ends_with("-docs.json"))
        .collect();
    files.sort();
    for file in files {
        let kind = file.trim_end_matches("-docs.json").to_string();
        cases.push((format!("corpus: {kind}"), corpus(&file)));
    }

    let options = Options {
        id_prefix: "bezel".into(),
        ..Options::default()
    };
    let rows: Vec<serde_json::Value> = cases
        .iter()
        .map(|(name, inputs)| {
            let (ms, done) = time(inputs, &options);
            serde_json::json!({ "case": name, "diagrams": inputs.len(), "ms": ms, "done": done })
        })
        .collect();
    let engine = format!("bezel-mermaid {}", env!("CARGO_PKG_VERSION"));
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({ "engine": engine, "rows": rows }))
            .expect("JSON")
    );
}
