//! Adding a language after the build — what the registry exists to allow.
//!
//! Each test mutates the process-wide registry. nextest runs every test in its
//! own process, so they do not see each other's registrations.

mod fixture;

use std::path::Path;
use syntax::registry::{self, Entry, Known};

/// A language nothing was built with answers to its tag and its files once it
/// is registered.
#[test]
fn a_registered_language_answers_like_a_seeded_one() {
    assert!(registry::of_tag("gleam").is_none());
    assert!(registry::of_path(Path::new("main.gleam")).is_none());

    registry::register(Entry {
        name: "gleam",
        aliases: &["gleam"],
        files: &["gleam"],
        lang: None,
    });

    assert_eq!(registry::of_tag("gleam"), Some(Known::Named("gleam")));
    assert_eq!(
        registry::of_path(Path::new("src/main.gleam")),
        Some(Known::Named("gleam"))
    );
}

/// Registering a name the table already holds replaces that row. Two rows of
/// one name would make which one answers depend on insertion order.
#[test]
fn registering_a_held_name_replaces_rather_than_shadows() {
    registry::register(Entry {
        name: "svelte",
        aliases: &["svelte", "svelte5"],
        files: &["svelte"],
        lang: None,
    });

    assert_eq!(registry::of_tag("svelte5"), Some(Known::Named("svelte")));
    assert_eq!(
        registry::names().iter().filter(|n| **n == "svelte").count(),
        1
    );
}

/// A registration carrying a grammar is [`Known::Ready`], and replaces the
/// name-only row seeded under that name.
#[test]
fn a_registration_carrying_a_grammar_is_ready() {
    assert_eq!(registry::of_tag("css"), Some(Known::Named("css")));

    fixture::install();

    let known = registry::of_tag("css").expect("css was registered");
    assert_eq!(known, Known::Ready(&fixture::CSS));
    assert!(known.lang().is_some());
    assert_eq!(registry::ready(), vec!["css", "html"]);
}

/// Registering does not disturb the rows already there.
#[test]
fn the_seeded_rows_survive_a_registration() {
    let before = registry::names().len();

    registry::register(Entry {
        name: "gleam",
        aliases: &["gleam"],
        files: &["gleam"],
        lang: None,
    });

    assert_eq!(registry::names().len(), before + 1);
    assert_eq!(registry::of_tag("rs").map(Known::name), Some("rust"));
    assert_eq!(registry::of_tag("svelte"), Some(Known::Named("svelte")));
    assert_eq!(
        registry::of_path(Path::new("main.gleam")),
        Some(Known::Named("gleam"))
    );
}
