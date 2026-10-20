//! The SVG against Pele's, entry by entry: `node pele/render.mjs svg <dir>`
//! writes Pele's, and `MERMAID_PELE_SVG=<dir>` runs this. Numbers may differ
//! by a hundredth, where a libm rounds differently.

use std::{fs, path::Path};

use mermaid::{Metrics, Options, render};

/// Corpus files whose drawing names the renderer, so cannot match.
const SKIPPED: [&str; 1] = ["info-docs"];

fn tokens(svg: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut number = false;
    for c in svg.chars() {
        let digit = c.is_ascii_digit() || c == '.' || (c == '-' && !number);
        if digit != number && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        number = digit;
        current.push(c);
    }
    out.push(current);
    out
}

fn same(ours: &str, theirs: &str) -> bool {
    if ours == theirs {
        return true;
    }
    let (a, b) = (tokens(ours), tokens(theirs));
    a.len() == b.len()
        && a.iter().zip(&b).all(|(x, y)| {
            x == y
                || matches!((x.parse::<f64>(), y.parse::<f64>()), (Ok(p), Ok(q)) if (p - q).abs() <= 0.011)
        })
}

#[test]
#[ignore = "needs Pele's drawings: MERMAID_PELE_SVG=<dir>"]
fn matches_pele() {
    let root = std::env::var("MERMAID_PELE_SVG").expect("MERMAID_PELE_SVG");
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mermaid/corpus");
    let only = std::env::var("MERMAID_KIND").ok();
    let mut checked = 0;
    let mut failures = Vec::new();
    for entry in fs::read_dir(&corpus).expect("corpus") {
        let path = entry.expect("entry").path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if SKIPPED.contains(&stem.as_str())
            || only
                .as_ref()
                .is_some_and(|only| !stem.starts_with(only.as_str()))
        {
            continue;
        }
        // A corpus entry is a source, or a reference entry with its `src`.
        let entries: Vec<serde_json::Value> =
            serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("JSON");
        let sources: Vec<&str> = entries
            .iter()
            .map(|entry| {
                entry
                    .as_str()
                    .or_else(|| entry["src"].as_str())
                    .expect("a source")
            })
            .collect();
        for (i, source) in sources.iter().enumerate() {
            let dir = Path::new(&root).join(&stem);
            let ours = render(source, &Options::default(), &Metrics);
            match (fs::read_to_string(dir.join(format!("{i}.svg"))), ours) {
                (Ok(theirs), Ok(ours)) => {
                    let theirs = theirs.replace("pele", mermaid::PREFIX);
                    if !same(&ours.svg, &theirs) {
                        failures.push(format!(
                            "{stem} #{i}\n  pele: {theirs}\n  ours: {}",
                            ours.svg
                        ));
                    }
                }
                (Err(_), Err(_)) => {}
                (Ok(_), Err(error)) => failures.push(format!("{stem} #{i}: ours failed: {error}")),
                (Err(_), Ok(_)) => failures.push(format!("{stem} #{i}: pele failed, ours drew")),
            }
            checked += 1;
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {checked}:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
