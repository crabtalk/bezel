//! What the registry answers with no provider installed.
//!
//! This crate links no grammar, so every seeded row is [`Known::Named`] until
//! something registers one. nextest runs each test in its own process, so a
//! registration in one is invisible to the rest.

use std::path::Path;
use syntax::registry::{self, Known};

/// Nothing paints until a provider registers.
#[test]
fn every_seeded_row_is_named_and_none_is_ready() {
    assert!(registry::ready().is_empty());
    assert!(!registry::names().is_empty());
    for name in registry::names() {
        assert_eq!(
            registry::of_tag(name),
            Some(Known::Named(name)),
            "{name} claims a grammar this crate does not link"
        );
    }
}

/// A name nothing in the table claims stays nothing.
#[test]
fn an_unknown_tag_is_none() {
    assert!(registry::of_tag("gleam").is_none());
    assert!(registry::of_path(Path::new("notes.gleam")).is_none());
}

/// Tags carry whatever the fence wrote after them.
#[test]
fn a_tag_is_trimmed_and_case_folded() {
    assert_eq!(registry::of_tag("Rust").map(Known::name), Some("rust"));
    assert_eq!(
        registry::of_tag("rust {.numberLines}").map(Known::name),
        Some("rust")
    );
    assert_eq!(registry::of_tag("rust,foo").map(Known::name), Some("rust"));
}

/// Files resolve through the same table as fence tags.
#[test]
fn a_path_resolves_to_the_same_rows() {
    assert_eq!(
        registry::of_path(Path::new("src/main.rs")).map(Known::name),
        Some("rust")
    );
    assert_eq!(
        registry::of_path(Path::new("routes/+page.svelte")).map(Known::name),
        Some("svelte")
    );
}

/// A whole name beats an extension, and the longer extension beats the shorter.
#[test]
fn a_whole_name_beats_an_extension() {
    assert_eq!(
        registry::of_path(Path::new("Dockerfile")).map(Known::name),
        Some("dockerfile")
    );
    assert_eq!(
        registry::of_path(Path::new(".bashrc")).map(Known::name),
        Some("bash")
    );
    assert_eq!(
        registry::of_path(Path::new("App.tsx")).map(Known::name),
        Some("tsx")
    );
    assert_eq!(
        registry::of_path(Path::new("app.ts")).map(Known::name),
        Some("typescript")
    );
}
