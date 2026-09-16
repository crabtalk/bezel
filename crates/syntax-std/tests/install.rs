//! What `install` puts into the registry.

use std::path::Path;
use syntax::registry::{self, Known};

/// Before install the registry names these languages and paints none of them.
/// nextest runs each test in its own process, so this sees a fresh registry.
#[test]
fn nothing_paints_until_install() {
    assert!(registry::ready().is_empty());
    assert_eq!(registry::of_tag("rs"), Some(Known::Named("rust")));

    syntax_std::install();

    assert_eq!(registry::of_tag("rs").map(Known::name), Some("rust"));
    assert!(registry::of_tag("rs").and_then(Known::lang).is_some());
}

/// Every row this build carries paints, and answers to its files.
#[test]
fn every_carried_row_is_ready_and_paints() {
    syntax_std::install();

    for lang in syntax_std::LANGS {
        let known = registry::of_tag(lang.name).expect("a carried row answers to its own name");
        assert!(
            known.lang().is_some(),
            "{} registered without a lang",
            lang.name
        );
    }
    assert_eq!(registry::ready().len(), syntax_std::LANGS.len());
    assert_eq!(
        registry::of_path(Path::new("src/main.rs")).map(Known::name),
        Some("rust")
    );
    assert!(syntax::highlight("fn main() {}", "rust").is_some_and(|spans| !spans.is_empty()));
}

/// Installing twice replaces the same rows rather than doubling them.
#[test]
fn installing_twice_is_the_same_registry() {
    syntax_std::install();
    let once = registry::names().len();
    syntax_std::install();
    assert_eq!(registry::names().len(), once);
}

/// A language no provider here carries stays named and unpainted.
#[test]
fn install_leaves_the_rest_named() {
    syntax_std::install();

    assert_eq!(registry::of_tag("svelte"), Some(Known::Named("svelte")));
    assert_eq!(
        registry::of_path(Path::new("routes/+page.svelte")),
        Some(Known::Named("svelte"))
    );
}
