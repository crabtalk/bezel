//! The languages bezel can highlight: a row per grammar — the fence aliases
//! it answers to, the tree-sitter grammar, and its highlights query.

use std::sync::OnceLock;
use theme::HighlightKind;
use tree_sitter::Language;
use tree_sitter_highlight::HighlightConfiguration;
use tree_sitter_language::LanguageFn;

pub struct Lang {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub grammar: LanguageFn,
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
    pub fn compiled(&'static self) -> Option<&'static Compiled> {
        self.compiled
            .get_or_init(|| {
                let grammar: Language = self.grammar.into();
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

pub static LANGS: [Lang; 8] = [
    Lang {
        name: "rust",
        aliases: &["rust", "rs"],
        grammar: tree_sitter_rust::LANGUAGE,
        query: include_str!("../queries/rust.scm"),
        compiled: OnceLock::new(),
    },
    Lang {
        name: "python",
        aliases: &["python", "py"],
        grammar: tree_sitter_python::LANGUAGE,
        query: include_str!("../queries/python.scm"),
        compiled: OnceLock::new(),
    },
    Lang {
        name: "typescript",
        aliases: &["typescript", "ts"],
        grammar: tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        query: include_str!("../queries/typescript.scm"),
        compiled: OnceLock::new(),
    },
    // JavaScript rides the TSX grammar: TSX parses JS, and a separate grammar
    // plus query would buy only the `<`-ambiguity edge cases that JSX and type
    // assertions disagree on — which a highlighted sample does not hinge on.
    Lang {
        name: "tsx",
        aliases: &["tsx", "jsx", "javascript", "js"],
        grammar: tree_sitter_typescript::LANGUAGE_TSX,
        query: include_str!("../queries/tsx.scm"),
        compiled: OnceLock::new(),
    },
    Lang {
        name: "json",
        aliases: &["json", "jsonc"],
        grammar: tree_sitter_json::LANGUAGE,
        query: include_str!("../queries/json.scm"),
        compiled: OnceLock::new(),
    },
    Lang {
        name: "go",
        aliases: &["go", "golang"],
        grammar: tree_sitter_go::LANGUAGE,
        query: include_str!("../queries/go.scm"),
        compiled: OnceLock::new(),
    },
    Lang {
        name: "bash",
        aliases: &["bash", "sh", "shell", "zsh", "console"],
        grammar: tree_sitter_bash::LANGUAGE,
        query: include_str!("../queries/bash.scm"),
        compiled: OnceLock::new(),
    },
    Lang {
        name: "toml",
        aliases: &["toml"],
        grammar: tree_sitter_toml_ng::LANGUAGE,
        query: include_str!("../queries/toml.scm"),
        compiled: OnceLock::new(),
    },
];

/// Find the language a fence tag names. Tags are the raw first word of the
/// fence info string — `rust {.numberLines}`, `rust,foo`, `Rust` — so they
/// are trimmed at the first space or comma and case-folded before lookup.
pub fn resolve(tag: &str) -> Option<&'static Lang> {
    let tag = tag
        .split([' ', ','])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    LANGS.iter().find(|l| l.aliases.contains(&tag.as_str()))
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
