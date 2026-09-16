//! Which languages this process can name, and which of those it can paint.
//!
//! [`LANGS`] seeds the registry; [`register`] adds to it at runtime. A name with
//! no grammar is still an entry — [`Known::Named`] — so a caller can tell a
//! language this build cannot paint from a file extension nobody has written a
//! grammar for.
//!
//! Registrations are leaked. A language lives for the process, and the strings
//! are a few hundred bytes against grammar bytes that are refcounted.

use crate::lang::{LANGS, Lang};
use std::{
    fmt,
    path::Path,
    sync::{OnceLock, RwLock},
};

/// What the registry knows about a name.
#[derive(Clone, Copy)]
pub enum Known {
    /// A grammar and a query are here.
    Ready(&'static Lang),
    /// Named, with nothing in this build to paint it.
    Named(&'static str),
}

/// `Ready` compares by identity: entries hold `&'static Lang`, and two rows
/// with the same name are the same row.
impl PartialEq for Known {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ready(held), Self::Ready(other)) => std::ptr::eq(*held, *other),
            (Self::Named(held), Self::Named(other)) => held == other,
            _ => false,
        }
    }
}

impl Eq for Known {}

impl fmt::Debug for Known {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready(lang) => write!(f, "Ready({})", lang.name),
            Self::Named(name) => write!(f, "Named({name})"),
        }
    }
}

impl Known {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ready(lang) => lang.name,
            Self::Named(name) => name,
        }
    }

    pub fn lang(self) -> Option<&'static Lang> {
        match self {
            Self::Ready(lang) => Some(lang),
            Self::Named(_) => None,
        }
    }
}

/// One language the registry knows of.
pub struct Entry {
    pub name: &'static str,
    /// Fence tags this answers to, lowercase.
    pub aliases: &'static [&'static str],
    /// Whole file names and extensions — `Dockerfile` is a name, `rs` an
    /// extension. [`of_path`] tries whole names first.
    pub files: &'static [&'static str],
    /// `None` names a language this build cannot paint.
    pub lang: Option<&'static Lang>,
}

impl Entry {
    fn known(&self) -> Known {
        match self.lang {
            Some(lang) => Known::Ready(lang),
            None => Known::Named(self.name),
        }
    }
}

fn registry() -> &'static RwLock<Vec<Entry>> {
    static REGISTRY: OnceLock<RwLock<Vec<Entry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(seeded()))
}

/// The rows this build carries, then the rows it can only name.
fn seeded() -> Vec<Entry> {
    let mut entries: Vec<Entry> = LANGS
        .iter()
        .map(|lang| Entry {
            name: lang.name,
            aliases: lang.aliases,
            files: files_for(lang.name),
            lang: Some(*lang),
        })
        .collect();
    let carried: Vec<&str> = entries.iter().map(|entry| entry.name).collect();
    entries.extend(NAMED.iter().filter(|(name, ..)| !carried.contains(name)).map(
        |(name, aliases, files)| Entry {
            name,
            aliases,
            files,
            lang: None,
        },
    ));
    entries
}

/// Add a language, or replace the entry of the same name. Leaks: see the module
/// note.
pub fn register(entry: Entry) {
    let Ok(mut entries) = registry().write() else {
        return;
    };
    entries.retain(|held| held.name != entry.name);
    entries.push(entry);
}

/// The language a fence tag names. Tags are the raw first word of the fence
/// info string — `rust {.numberLines}`, `rust,foo`, `Rust` — so they are
/// trimmed at the first space or comma and case-folded before lookup.
pub fn of_tag(tag: &str) -> Option<Known> {
    let tag = tag
        .split([' ', ','])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let entries = registry().read().ok()?;
    entries
        .iter()
        .find(|entry| entry.aliases.contains(&tag.as_str()))
        .map(Entry::known)
}

/// The language `path` is written in.
///
/// A whole name beats an extension — `Dockerfile` is not a `.file` — and
/// between extensions the longest match wins, so `.d.ts` would beat `.ts`
/// rather than racing it.
pub fn of_path(path: &Path) -> Option<Known> {
    let name = path.file_name()?.to_str()?;
    let entries = registry().read().ok()?;
    entries
        .iter()
        .filter_map(|entry| {
            entry
                .files
                .iter()
                .filter(|candidate| matches(name, candidate))
                .map(|candidate| (candidate.len(), entry))
                .max_by_key(|(len, _)| *len)
        })
        .max_by_key(|(len, _)| *len)
        .map(|(_, entry)| entry.known())
}

fn matches(name: &str, candidate: &str) -> bool {
    name.eq_ignore_ascii_case(candidate)
        || name.len() > candidate.len()
            && name[name.len() - candidate.len() - 1..]
                .eq_ignore_ascii_case(&format!(".{candidate}"))
}

/// Every name the registry holds, painted or not.
pub fn names() -> Vec<&'static str> {
    let Ok(entries) = registry().read() else {
        return Vec::new();
    };
    entries.iter().map(|entry| entry.name).collect()
}

/// Every name this build can paint.
pub fn ready() -> Vec<&'static str> {
    let Ok(entries) = registry().read() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter(|entry| entry.lang.is_some())
        .map(|entry| entry.name)
        .collect()
}

/// File names and extensions for the grammars a build can carry, keyed by the
/// [`Lang::name`] they belong to.
const BUILTIN_FILES: &[(&str, &[&str])] = &[
    (
        "bash",
        &[
            ".bash_profile",
            ".bashrc",
            ".profile",
            ".zshrc",
            "bash",
            "sh",
            "zsh",
        ],
    ),
    ("css", &["css", "scss"]),
    ("go", &["go"]),
    ("html", &["htm", "html"]),
    ("json", &["json", "jsonc"]),
    ("python", &["py", "pyi"]),
    ("rust", &["rs"]),
    ("toml", &["toml"]),
    ("tsx", &["cjs", "jsx", "mjs", "tsx"]),
    ("typescript", &["cts", "mts", "ts"]),
];

fn files_for(name: &str) -> &'static [&'static str] {
    BUILTIN_FILES
        .iter()
        .find(|(held, _)| *held == name)
        .map(|(_, files)| *files)
        .unwrap_or(&[])
}

/// Languages with no grammar in any build of this crate yet: name, fence tags,
/// then file names and extensions.
#[rustfmt::skip]
const NAMED: &[(&str, &[&str], &[&str])] = &[
    ("c", &["c"], &["c", "h"]),
    ("cpp", &["cpp", "c++"], &["cc", "cpp", "cxx", "hpp"]),
    ("csharp", &["csharp", "cs"], &["cs"]),
    ("dockerfile", &["dockerfile"], &["Containerfile", "Dockerfile"]),
    ("elixir", &["elixir", "ex"], &["ex", "exs"]),
    ("graphql", &["graphql", "gql"], &["gql", "graphql"]),
    ("haskell", &["haskell", "hs"], &["hs"]),
    ("java", &["java"], &["java"]),
    ("kotlin", &["kotlin", "kt"], &["kt", "kts"]),
    ("lua", &["lua"], &["lua"]),
    ("make", &["make", "makefile"], &["Makefile", "mk"]),
    ("markdown", &["markdown", "md"], &["markdown", "md"]),
    ("nix", &["nix"], &["nix"]),
    ("php", &["php"], &["php"]),
    ("proto", &["proto", "protobuf"], &["proto"]),
    ("ruby", &["ruby", "rb"], &["Gemfile", "Rakefile", "erb", "rb"]),
    ("scala", &["scala"], &["sbt", "scala"]),
    ("sql", &["sql"], &["sql"]),
    ("svelte", &["svelte"], &["svelte"]),
    ("swift", &["swift"], &["swift"]),
    ("vue", &["vue"], &["vue"]),
    ("xml", &["xml"], &["xml"]),
    ("yaml", &["yaml", "yml"], &["yaml", "yml"]),
    ("zig", &["zig"], &["zig"]),
];
