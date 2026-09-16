//! Adding a language after the build — what the registry exists to allow.
//!
//! Each test mutates the process-wide registry. nextest runs every test in its
//! own process, so they do not see each other's registrations.

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

/// A registration carrying a grammar is [`Known::Ready`], and paints.
#[test]
fn a_registration_carrying_a_grammar_is_ready() {
    let rust = registry::of_tag("rs")
        .and_then(Known::lang)
        .expect("rust is seeded");

    registry::register(Entry {
        name: "rust-script",
        aliases: &["rust-script"],
        files: &["rs-script"],
        lang: Some(rust),
    });

    let known = registry::of_tag("rust-script").expect("just registered");
    assert_eq!(known, Known::Ready(rust));
    assert!(known.lang().is_some());
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
}
