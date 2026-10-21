//! Vega-Lite's example gallery at v6.5.0, `examples/specs` in `specs/`. The
//! specs that import, which a spec does only where it draws, are exactly
//! those named in `pass.txt`. What Vega draws
//! for each is written by `node vega/render.mjs <dir>`.
//!
//! `data/` holds the vega-datasets 3.2.1 files the specs name, with its
//! `datapackage.json` giving each one's source and license. A spec's
//! `data/<file>` url, and a jsDelivr url into vega-datasets, is loaded from
//! there.

use chart::vega_lite::Spec;
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::Path,
};

const JSDELIVR: &str = "https://cdn.jsdelivr.net/npm/vega-datasets@";

#[test]
fn the_specs_that_import_are_the_pass_list() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vega_lite");
    let listed: BTreeSet<String> = fs::read_to_string(root.join("pass.txt"))
        .expect("pass list")
        .lines()
        .map(str::to_owned)
        .collect();
    let files: HashMap<String, String> = fs::read_dir(root.join("data"))
        .expect("data")
        .map(|entry| {
            let path = entry.expect("data file").path();
            let name = path.file_name().unwrap().to_str().unwrap().to_owned();
            (name, fs::read_to_string(&path).expect("data file"))
        })
        .collect();
    let load = |url: &str| {
        let path = match url.strip_prefix(JSDELIVR) {
            Some(rest) => rest.split_once('/')?.1,
            None => url,
        };
        files.get(path.strip_prefix("data/")?).map(String::as_str)
    };
    let mut passing = BTreeSet::new();
    for entry in fs::read_dir(root.join("specs")).expect("specs") {
        let path = entry.expect("spec").path();
        let name = path.file_name().unwrap().to_str().unwrap();
        let Some(name) = name.strip_suffix(".vl.json") else {
            continue;
        };
        let spec: Spec = fs::read_to_string(&path)
            .expect("spec")
            .parse()
            .expect("spec JSON");
        if spec.chart(&load).is_some() {
            passing.insert(name.to_owned());
        }
    }
    let gained: Vec<_> = passing.difference(&listed).collect();
    let lost: Vec<_> = listed.difference(&passing).collect();
    assert!(
        gained.is_empty() && lost.is_empty(),
        "now import, add to pass.txt: {gained:?}\nno longer import: {lost:?}"
    );
}
