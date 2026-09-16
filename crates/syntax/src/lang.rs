//! The languages bezel can highlight: a row per grammar — the fence aliases
//! it answers to, the tree-sitter grammar, and its highlights query. Each row
//! is behind the feature of the same name.

use std::{ops::Range, sync::Arc};
use theme::HighlightKind;
use tree_sitter::Language;
use tree_sitter_highlight::HighlightConfiguration;
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
    /// Which regions are written in another language, and which. Empty is a
    /// document parsed with one grammar throughout.
    pub injections: &'static str,
}

/// Every capture name [`kind_of`] answers to.
///
/// One list for every language, because a `Highlight` index means whatever the
/// layer that produced it was configured with — an injected CSS parse inside
/// html returns indices its own query decided. Configuring each `Lang` with its
/// own captures makes those indices mean different things per layer.
pub const NAMES: &[&str] = &[
    "comment",
    "comment.documentation",
    "keyword",
    "keyword.function",
    "keyword.return",
    "keyword.operator",
    "keyword.conditional",
    "keyword.conditional.ternary",
    "keyword.coroutine",
    "keyword.directive",
    "keyword.exception",
    "keyword.import",
    "keyword.modifier",
    "keyword.repeat",
    "keyword.type",
    "string",
    "string.special",
    "string.special.key",
    "string.special.url",
    "string.regexp",
    "character.special",
    "escape",
    "string.escape",
    "number",
    "boolean",
    "type",
    "type.interface",
    "type.builtin",
    "constructor",
    "function",
    "function.method",
    "function.method.call",
    "function.call",
    "function.builtin",
    "macro",
    "function.macro",
    "property",
    "property.definition",
    "variable.member",
    "constant",
    "constant.builtin",
    "module",
    "module.builtin",
    "variable",
    "variable.builtin",
    "variable.special",
    "self",
    "variable.parameter",
    "parameter",
    "operator",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation.special",
    "tag",
    "tag.builtin",
    "tag.delimiter",
    "attribute",
    "tag.attribute",
    "label",
    "invalid",
];

/// A grammar's query, compiled and configured. Held by a
/// [`Session`](crate::session::Session), never shared between two.
pub struct Compiled {
    pub config: HighlightConfiguration,
    /// Languages named by a `#set! injection.language` in the query, so a
    /// session can compile them before a parse needs them.
    pub injected: Vec<String>,
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
            injections: "",
        }
    }

    /// Give this language an injections query. `@injection.content` marks the
    /// region and `@injection.language` names what it is written in; the name
    /// is resolved through [`crate::registry`], so an injected language must be
    /// one the registry can paint.
    pub const fn with_injections(mut self, injections: &'static str) -> Self {
        self.injections = injections;
        self
    }

    /// Spans over `source`, through this thread's
    /// [`Session`](crate::session::Session).
    pub fn highlight(&'static self, source: &str) -> Option<Vec<(Range<usize>, HighlightKind)>> {
        crate::session::with(|session| session.highlight(self, source))
    }

    /// Compile this language's queries against `grammar`, which the session
    /// supplies because a wasm one comes from its store. `None` where a query
    /// does not compile.
    pub(crate) fn compile_with(&self, grammar: Language) -> Option<Compiled> {
        let mut config =
            HighlightConfiguration::new(grammar, self.name, self.query, self.injections, "").ok()?;
        config.configure(NAMES);
        let injected = (0..config.query.pattern_count())
            .flat_map(|pattern| config.query.property_settings(pattern))
            .filter(|property| property.key.as_ref() == "injection.language")
            .filter_map(|property| property.value.as_ref().map(|value| value.to_string()))
            .collect();
        Some(Compiled { config, injected })
    }
}

/// Find the language a fence tag names, where this build carries a grammar for
/// it. [`crate::registry::of_tag`] answers for a tag it can only name.
pub fn resolve(tag: &str) -> Option<&'static Lang> {
    crate::registry::of_tag(tag)?.lang()
}

/// Map a tree-sitter highlight capture name onto the bezel vocabulary.
///
/// [`NAMES`] is this function's domain. A capture outside it is never
/// configured, so it produces no span and its text is left plain.
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
