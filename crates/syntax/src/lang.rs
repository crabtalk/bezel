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
    /// Which regions are written in another language, and which. Empty is a
    /// document parsed with one grammar throughout.
    pub injections: &'static str,
    /// Compiling a highlights query costs milliseconds — tsx.scm is 750 lines
    /// — and a render loop calls [`Lang::compiled`] every frame.
    compiled: OnceLock<Option<Compiled>>,
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

/// A grammar's query, compiled and configured once.
pub struct Compiled {
    pub config: HighlightConfiguration,
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
            compiled: OnceLock::new(),
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

    /// Spans over `source`, in bytes, in document order. `None` when the query
    /// does not compile against the grammar.
    // The callback is a closure rather than `injected` itself: as a fn item its
    // return type unifies with the callback's `'a`, binding it to `'static` and
    // requiring `source` and the local highlighter to outlive the call.
    #[allow(clippy::redundant_closure)]
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
            .highlight(config, source.as_bytes(), None, None, |name| injected(name))
            .ok()?
            .flatten()
        {
            match event {
                HighlightEvent::HighlightStart(hl) => {
                    kinds.push(kind_of(NAMES.get(hl.0).copied().unwrap_or("")));
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
                    HighlightConfiguration::new(grammar, self.name, self.query, self.injections, "")
                        .ok()?;
                config.configure(NAMES);
                Some(Compiled { config })
            })
            .as_ref()
    }
}

/// The configuration an injected language is painted with.
///
/// Every `Lang` outlives the parse that borrows it, so an injected layer needs
/// no lifetime of its own — which stops holding once a grammar is loaded into a
/// per-parser store rather than linked.
fn injected(name: &str) -> Option<&'static HighlightConfiguration> {
    let lang = crate::registry::of_tag(name)?.lang()?;
    Some(&lang.compiled()?.config)
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
