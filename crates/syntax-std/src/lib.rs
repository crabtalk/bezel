//! The grammars bezel links in, as one provider for `syntax::registry`.
//!
//! [`install`] puts every row this build carries into the registry. Nothing
//! registers itself — an app calls `install` once, and may register more from
//! anywhere else before or after.

use syntax::{
    lang::{Grammar, Lang},
    registry::{self, Entry},
};

/// Register every grammar this build carries. Calling it twice replaces the
/// same rows rather than duplicating them.
pub fn install() {
    for lang in LANGS {
        registry::register(Entry {
            name: lang.name,
            aliases: lang.aliases,
            files: files_for(lang.name),
            lang: Some(lang),
        });
    }
}

fn files_for(name: &str) -> &'static [&'static str] {
    FILES
        .iter()
        .find(|(held, _)| *held == name)
        .map(|(_, files)| *files)
        .unwrap_or(&[])
}

/// File names and extensions for the grammars here. Fence aliases live on each
/// [`Lang`]; these are what a path is matched against.
#[rustfmt::skip]
const FILES: &[(&str, &[&str])] = &[
    ("bash", &[".bash_profile", ".bashrc", ".profile", ".zshrc", "bash", "sh", "zsh"]),
    ("go", &["go"]),
    ("json", &["json", "jsonc"]),
    ("python", &["py", "pyi"]),
    ("rust", &["rs"]),
    ("toml", &["toml"]),
    ("tsx", &["cjs", "jsx", "mjs", "tsx"]),
    ("typescript", &["cts", "mts", "ts"]),
];

#[cfg(feature = "rust")]
static RUST: Lang = Lang::new(
    "rust",
    &["rust", "rs"],
    Grammar::Native(tree_sitter_rust::LANGUAGE),
    include_str!("../queries/rust.scm"),
);
#[cfg(feature = "python")]
static PYTHON: Lang = Lang::new(
    "python",
    &["python", "py"],
    Grammar::Native(tree_sitter_python::LANGUAGE),
    include_str!("../queries/python.scm"),
);
#[cfg(feature = "typescript")]
static TYPESCRIPT: Lang = Lang::new(
    "typescript",
    &["typescript", "ts"],
    Grammar::Native(tree_sitter_typescript::LANGUAGE_TYPESCRIPT),
    include_str!("../queries/typescript.scm"),
);
/// JavaScript rides the TSX grammar: TSX parses JS, and a separate grammar plus
/// query would buy only the `<`-ambiguity edge cases that JSX and type
/// assertions disagree on — which a highlighted sample does not hinge on.
#[cfg(feature = "typescript")]
static TSX: Lang = Lang::new(
    "tsx",
    &["tsx", "jsx", "javascript", "js"],
    Grammar::Native(tree_sitter_typescript::LANGUAGE_TSX),
    include_str!("../queries/tsx.scm"),
);
#[cfg(feature = "json")]
static JSON: Lang = Lang::new(
    "json",
    &["json", "jsonc"],
    Grammar::Native(tree_sitter_json::LANGUAGE),
    include_str!("../queries/json.scm"),
);
#[cfg(feature = "go")]
static GO: Lang = Lang::new(
    "go",
    &["go", "golang"],
    Grammar::Native(tree_sitter_go::LANGUAGE),
    include_str!("../queries/go.scm"),
);
#[cfg(feature = "bash")]
static BASH: Lang = Lang::new(
    "bash",
    &["bash", "sh", "shell", "zsh", "console"],
    Grammar::Native(tree_sitter_bash::LANGUAGE),
    include_str!("../queries/bash.scm"),
);
#[cfg(feature = "toml")]
static TOML: Lang = Lang::new(
    "toml",
    &["toml"],
    Grammar::Native(tree_sitter_toml_ng::LANGUAGE),
    include_str!("../queries/toml.scm"),
);

/// A slice rather than an array: its length is whatever the enabled features
/// add up to, and an app that highlights one language compiles one grammar.
pub static LANGS: &[&Lang] = &[
    #[cfg(feature = "rust")]
    &RUST,
    #[cfg(feature = "python")]
    &PYTHON,
    #[cfg(feature = "typescript")]
    &TYPESCRIPT,
    #[cfg(feature = "typescript")]
    &TSX,
    #[cfg(feature = "json")]
    &JSON,
    #[cfg(feature = "go")]
    &GO,
    #[cfg(feature = "bash")]
    &BASH,
    #[cfg(feature = "toml")]
    &TOML,
];
