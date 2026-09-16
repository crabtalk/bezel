//! The languages bezel can highlight: a row per grammar — the fence aliases
//! it answers to, the tree-sitter grammar, and its highlights query. Each row
//! is behind the feature of the same name.

use std::{ops::Range, sync::Arc, sync::OnceLock};
use theme::HighlightKind;
use tree_sitter::Language;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};
use tree_sitter_language::LanguageFn;

/// Where a grammar's parse tables come from.
///
/// Both variants compile in every configuration — `Wasm` carries bytes, which
/// need no engine. Loading them does, and that is behind the `wasm` feature.
pub enum Grammar {
    /// Linked at build time.
    Native(LanguageFn),
    /// A compiled module, held as bytes: a wasm `Language` belongs to the
    /// `WasmStore` that loaded it, so one cannot be made here.
    Wasm(Arc<[u8]>),
}

impl Grammar {
    /// The tree-sitter language, where one can be made without a store.
    /// [`Grammar::Wasm`] needs a `WasmStore` and answers `None`.
    pub fn language(&self) -> Option<Language> {
        match self {
            Self::Native(grammar) => Some((*grammar).into()),
            Self::Wasm(_) => None,
        }
    }
}

pub struct Lang {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub grammar: Grammar,
    pub query: &'static str,
    /// Compiling a highlights query costs milliseconds — tsx.scm is 750 lines
    /// — and a render loop calls [`Lang::compiled`] every frame.
    compiled: OnceLock<Option<Compiled>>,
}

/// A grammar's query, compiled and configured once.
pub struct Compiled {
    pub config: HighlightConfiguration,
    /// Capture names by `Highlight` index, for [`kind_of`].
    pub names: Vec<String>,
}

impl Lang {
    /// A language of your own: a grammar, its highlights query, and the fence
    /// tags it answers to. `const`, so it can be a `static` beside the built-in
    /// rows.
    pub const fn new(
        name: &'static str,
        aliases: &'static [&'static str],
        grammar: Grammar,
        query: &'static str,
    ) -> Self {
        Self {
            name,
            aliases,
            grammar,
            query,
            compiled: OnceLock::new(),
        }
    }

    /// Spans over `source`, in bytes, in document order. `None` when the query
    /// does not compile against the grammar.
    pub fn highlight(&'static self, source: &str) -> Option<Vec<(Range<usize>, HighlightKind)>> {
        let compiled = self.compiled()?;
        let config = &compiled.config;
        let mut highlighter = Highlighter::new();
        highlighter.parser().set_language(&config.language).ok()?;
        let mut spans = Vec::new();
        // Nested highlight starts end with `HighlightEnd`; the top of the stack
        // is the kind painting the `Source` ranges that follow it.
        let mut kinds: Vec<HighlightKind> = Vec::new();
        for event in highlighter
            // `None` encoding: the source is a `&str`, so it is UTF-8 and
            // tree-sitter's default is the one to take.
            .highlight(config, source.as_bytes(), None, None, |_| None)
            .ok()?
            .flatten()
        {
            match event {
                HighlightEvent::HighlightStart(hl) => {
                    let name = compiled.names.get(hl.0).map(String::as_str).unwrap_or("");
                    kinds.push(kind_of(name));
                }
                HighlightEvent::HighlightEnd => {
                    kinds.pop();
                }
                HighlightEvent::Source { start, end } => {
                    if let Some(&kind) = kinds.last() {
                        spans.push((start..end, kind));
                    }
                }
            }
        }
        Some(spans)
    }

    pub fn compiled(&'static self) -> Option<&'static Compiled> {
        self.compiled
            .get_or_init(|| {
                let grammar = self.grammar.language()?;
                let mut config =
                    HighlightConfiguration::new(grammar, self.name, self.query, "", "").ok()?;
                // Recognize exactly the capture names the query uses, so every
                // `Highlight` index resolves straight through `names`.
                // `_`-prefixed names are predicate anchors, never paint —
                // recognizing them would emit their ranges as spans.
                let names: Vec<String> = config
                    .query
                    .capture_names()
                    .iter()
                    .map(|s| s.to_string())
                    .filter(|s| !s.starts_with('_'))
                    .collect();
                config.configure(&names);
                Some(Compiled { config, names })
            })
            .as_ref()
    }
}

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

/// Find the language a fence tag names, where this build carries a grammar for
/// it. [`crate::registry::of_tag`] answers for a tag it can only name.
pub fn resolve(tag: &str) -> Option<&'static Lang> {
    crate::registry::of_tag(tag)?.lang()
}

/// Map a tree-sitter highlight capture name onto the bezel vocabulary. Names
/// with no slot degrade to [`HighlightKind::Variable`], which the palettes
/// paint in the text color — an unknown capture reads as plain text.
pub fn kind_of(name: &str) -> HighlightKind {
    match name {
        "comment" | "comment.documentation" => HighlightKind::Comment,
        "keyword"
        | "keyword.function"
        | "keyword.return"
        | "keyword.operator"
        | "keyword.conditional"
        | "keyword.conditional.ternary"
        | "keyword.coroutine"
        | "keyword.directive"
        | "keyword.exception"
        | "keyword.import"
        | "keyword.modifier"
        | "keyword.repeat"
        | "keyword.type" => HighlightKind::Keyword,
        "string" => HighlightKind::String,
        "string.special" | "string.special.key" | "string.special.url" | "string.regexp"
        | "character.special" => HighlightKind::StringSpecial,
        "escape" | "string.escape" => HighlightKind::Escape,
        "number" => HighlightKind::Number,
        "boolean" => HighlightKind::Boolean,
        "type" | "type.interface" => HighlightKind::TypeName,
        "type.builtin" => HighlightKind::TypeBuiltin,
        "constructor" => HighlightKind::Constructor,
        "function" | "function.method" | "function.method.call" | "function.call" => {
            HighlightKind::Function
        }
        "function.builtin" => HighlightKind::FunctionBuiltin,
        "macro" | "function.macro" => HighlightKind::MacroName,
        "property" | "property.definition" | "variable.member" => HighlightKind::Property,
        "constant" | "constant.builtin" | "module" | "module.builtin" => HighlightKind::Constant,
        "variable" | "variable.builtin" => HighlightKind::Variable,
        "variable.special" | "self" => HighlightKind::VariableSpecial,
        "variable.parameter" | "parameter" => HighlightKind::Parameter,
        "operator" => HighlightKind::Operator,
        "punctuation" | "punctuation.bracket" | "punctuation.delimiter" | "punctuation.special" => {
            HighlightKind::Punctuation
        }
        "tag" | "tag.builtin" => HighlightKind::Tag,
        "tag.delimiter" => HighlightKind::Punctuation,
        "attribute" | "tag.attribute" => HighlightKind::Attribute,
        "label" => HighlightKind::Label,
        "invalid" => HighlightKind::Invalid,
        _ => HighlightKind::Variable,
    }
}
