//! What the registry answers for a name, and what it answers for a file.

use std::path::Path;
use syntax::registry::{self, Known};

/// A grammar this build carries resolves to the row that holds it.
#[test]
fn a_carried_language_is_ready() {
    let known = registry::of_tag("rs").expect("rust is a seeded row");
    assert!(matches!(known, Known::Ready(_)));
    assert_eq!(known.name(), "rust");
    assert!(known.lang().is_some());
}

/// The distinction the whole registry exists for: a language the build can name
/// and not paint is an entry, not a miss.
#[test]
fn a_language_with_no_grammar_is_named_rather_than_missing() {
    let known = registry::of_tag("svelte").expect("svelte is a named row");
    assert_eq!(known, Known::Named("svelte"));
    assert!(known.lang().is_none());
}

/// A name nothing in the table claims stays nothing.
#[test]
fn an_unknown_tag_is_none() {
    assert!(registry::of_tag("gleam").is_none());
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
        registry::of_path(Path::new("+page.svelte")),
        Some(Known::Named("svelte"))
    );
    assert!(registry::of_path(Path::new("notes.gleam")).is_none());
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
}

/// Every seeded row that claims a grammar has one behind it.
#[test]
fn every_ready_row_carries_a_lang() {
    for name in registry::ready() {
        let known = registry::of_tag(name).expect("a ready row answers to its own name");
        assert!(known.lang().is_some(), "{name} is ready without a lang");
    }
}

/// Named rows outnumber carried ones, and both are in one table.
#[test]
fn the_table_holds_more_names_than_grammars() {
    assert!(registry::names().len() > registry::ready().len());
}
