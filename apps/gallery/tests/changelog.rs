//! `changelog.json` is what the release notes and the website are both built
//! from, so a version that shipped without an entry is a release with nothing
//! to say for itself. Asserted here rather than remembered, the way the rail's
//! source paths are.

use std::{fs, path::PathBuf};

fn releases() -> Vec<serde_json::Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../changelog.json");
    let raw = fs::read_to_string(&path).expect("changelog.json is at the repo root");
    serde_json::from_str(&raw).expect("changelog.json is a JSON array of releases")
}

fn field<'a>(release: &'a serde_json::Value, key: &str) -> &'a str {
    release[key]
        .as_str()
        .unwrap_or_else(|| panic!("a release has a {key}"))
}

/// The workspace version — every member inherits it, so this crate's is it.
#[test]
fn the_version_being_shipped_has_an_entry() {
    let version = env!("CARGO_PKG_VERSION");
    assert!(
        releases()
            .iter()
            .any(|release| field(release, "version") == version),
        "no changelog.json entry for {version} — add one before the bump lands"
    );
}

#[test]
fn every_release_says_what_it_is_and_when() {
    for release in releases() {
        let version = field(&release, "version").to_string();
        let date = field(&release, "date");
        assert!(
            date.len() == 10 && date.split('-').count() == 3,
            "{version}'s date is not `YYYY-MM-DD`: {date:?}"
        );
        assert!(
            !field(&release, "summary").is_empty(),
            "{version} has no summary"
        );
        // The three groups the notes render, in this order, and only the ones a
        // release has. Anything else is a key the renderer would drop silently.
        let groups = ["new", "changed", "fixed"];
        let object = release.as_object().expect("a release is an object");
        for key in object.keys() {
            assert!(
                ["version", "date", "summary"].contains(&key.as_str())
                    || groups.contains(&key.as_str()),
                "{version} carries an unknown key {key:?}"
            );
        }
        assert!(
            groups.iter().any(|key| object.contains_key(*key)),
            "{version} lists no changes at all"
        );
        for key in groups {
            if let Some(entries) = object.get(key) {
                let entries = entries
                    .as_array()
                    .unwrap_or_else(|| panic!("{version}'s {key} is a list"));
                assert!(
                    !entries.is_empty(),
                    "{version}'s {key} is empty rather than absent"
                );
                assert!(
                    entries
                        .iter()
                        .all(|entry| entry.as_str().is_some_and(|s| !s.is_empty())),
                    "{version}'s {key} holds something that is not a line of prose"
                );
            }
        }
    }
}

/// Newest first, the way the notes read and the way the site lists them.
#[test]
fn releases_are_ordered_newest_first() {
    let parts = |version: &str| {
        version
            .split('.')
            .map(|part| part.parse::<u32>().expect("a numeric version part"))
            .collect::<Vec<_>>()
    };
    let versions: Vec<_> = releases()
        .iter()
        .map(|release| parts(field(release, "version")))
        .collect();
    assert!(
        versions.windows(2).all(|pair| pair[0] > pair[1]),
        "changelog.json is out of order: {versions:?}"
    );
}
