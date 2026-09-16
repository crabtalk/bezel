//! Where a parse happens: one parser, and the queries compiled against it.
//!
//! Configs live here rather than on [`Lang`] because a wasm `Language` belongs
//! to the `WasmStore` its parser holds — `parser.c` dispatches every lex and
//! external-scanner call through that store — so a config compiled for one
//! parser cannot be shared with another.
//!
//! An injected language's config must already be in [`Session::configs`] when
//! the parse starts: the callback holds `configs` immutably and cannot compile
//! into it. [`Session::ensure`] walks the `#set! injection.language` names of a
//! query before parsing and compiles each. An injection that names its language
//! through a captured node instead — the language is in the source text — is
//! only painted if that language was already compiled.

use crate::{
    lang::{Grammar, Lang},
    registry,
};
use std::{cell::RefCell, collections::HashMap, ops::Range};
use theme::HighlightKind;
use tree_sitter::Language;
use tree_sitter_highlight::{HighlightEvent, Highlighter};

/// A parser and its compiled queries.
#[derive(Default)]
pub struct Session {
    highlighter: Highlighter,
    /// By [`Lang::name`]. `None` is a query that did not compile against its
    /// grammar, cached so it is not retried on every parse.
    configs: HashMap<&'static str, Option<crate::lang::Compiled>>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compile `lang` and everything its injections name, transitively.
    fn ensure(&mut self, lang: &'static Lang) {
        if self.configs.contains_key(lang.name) {
            return;
        }
        let compiled = self.compile(lang);
        let injected = compiled
            .as_ref()
            .map(|compiled| compiled.injected.clone())
            .unwrap_or_default();
        // Recorded before recursing: two languages injecting each other would
        // otherwise not terminate.
        self.configs.insert(lang.name, compiled);
        for name in injected {
            if let Some(lang) = registry::of_tag(&name).and_then(|known| known.lang()) {
                self.ensure(lang);
            }
        }
    }

    /// The grammar for `lang`, then its queries compiled against it.
    fn compile(&mut self, lang: &'static Lang) -> Option<crate::lang::Compiled> {
        let grammar = match &lang.grammar {
            Grammar::Native(grammar) => (*grammar).into(),
            Grammar::Wasm(bytes) => self.load_wasm(lang.name, &bytes.clone())?,
        };
        lang.compile_with(grammar)
    }

    /// Instantiate a wasm grammar in this session's store, making one from the
    /// installed engine if there is not one yet.
    ///
    /// The store lives on the parser rather than beside it: `set_language` with
    /// a wasm language reads the store off the parser, and the highlighter calls
    /// it for every layer. Taking it back out is how a later language is loaded
    /// into the same store.
    #[cfg(feature = "wasm")]
    fn load_wasm(&mut self, name: &str, bytes: &[u8]) -> Option<Language> {
        use tree_sitter::WasmStore;

        let mut store = match self.highlighter.parser().take_wasm_store() {
            Some(store) => store,
            None => WasmStore::new(crate::engine()?).ok()?,
        };
        let language = store.load_language(name, bytes).ok();
        self.highlighter.parser().set_wasm_store(store).ok()?;
        language
    }

    /// Without the `wasm` feature there is no engine to instantiate in.
    #[cfg(not(feature = "wasm"))]
    fn load_wasm(&mut self, _name: &str, _bytes: &[u8]) -> Option<Language> {
        None
    }

    /// Spans over `source`, in bytes, in document order. `None` when the query
    /// does not compile against the grammar.
    pub fn highlight(
        &mut self,
        lang: &'static Lang,
        source: &str,
    ) -> Option<Vec<(Range<usize>, HighlightKind)>> {
        self.ensure(lang);
        // Split borrows: the parse holds `highlighter` mutably while the
        // callback reads `configs`.
        let Self {
            highlighter,
            configs,
        } = self;
        let config = &configs.get(lang.name)?.as_ref()?.config;
        let mut spans = Vec::new();
        // Nested highlight starts end with `HighlightEnd`; the top of the stack
        // is the kind painting the `Source` ranges that follow it.
        let mut kinds: Vec<HighlightKind> = Vec::new();
        for event in highlighter
            // `None` encoding: the source is a `&str`, so it is UTF-8 and
            // tree-sitter's default is the one to take.
            .highlight(config, source.as_bytes(), None, None, |name| {
                let key = registry::of_tag(name)?.lang()?.name;
                configs.get(key)?.as_ref().map(|compiled| &compiled.config)
            })
            .ok()?
            .flatten()
        {
            match event {
                HighlightEvent::HighlightStart(hl) => {
                    kinds.push(crate::lang::kind_of(
                        crate::lang::NAMES.get(hl.0).copied().unwrap_or(""),
                    ));
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
}

thread_local! {
    /// One per thread. A `Highlighter` owns a parser and a config costs
    /// milliseconds to compile, so neither is rebuilt per call.
    static SESSION: RefCell<Session> = RefCell::new(Session::new());
}

/// Run `f` against this thread's session.
pub fn with<T>(f: impl FnOnce(&mut Session) -> T) -> T {
    SESSION.with(|session| f(&mut session.borrow_mut()))
}
