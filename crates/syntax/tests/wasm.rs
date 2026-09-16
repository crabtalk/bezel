//! A grammar loaded as wasm rather than linked.
//!
//! The fixtures are tree-sitter grammars compiled to wasm. See
//! `tests/fixture/README.md` for what they are and how to rebuild them.

#![cfg(feature = "wasm")]

use std::sync::Arc;
use syntax::{
    lang::{Grammar, Lang},
    registry::{self, Entry, Known},
    session::Session,
    wasmtime,
};
use theme::HighlightKind;

const WASM: &[u8] = include_bytes!("fixture/json.wasm");
const CSS_WASM: &[u8] = include_bytes!("fixture/css.wasm");

const QUERY: &str = r#"
(string) @string
(number) @number
[(true) (false)] @boolean
(null) @constant.builtin
(pair key: (string) @property)
"#;

/// Build the language from bytes, the way a fetched grammar would arrive.
fn register() -> &'static Lang {
    let lang: &'static Lang = Box::leak(Box::new(Lang::new(
        "json",
        &["json"],
        Grammar::Wasm(Arc::from(WASM)),
        QUERY,
    )));
    registry::register(Entry {
        name: lang.name,
        aliases: lang.aliases,
        files: &["json"],
        lang: Some(lang),
    });
    lang
}

/// With no engine installed a wasm grammar cannot be instantiated, so the
/// language registers but paints nothing.
#[test]
fn without_an_engine_a_wasm_grammar_paints_nothing() {
    let lang = register();
    assert_eq!(registry::of_tag("json"), Some(Known::Ready(lang)));
    assert!(Session::new().highlight(lang, r#"{"a": 1}"#).is_none());
}

/// The whole point: bytes in, spans out, with nothing about this grammar linked
/// into the binary.
#[test]
fn a_wasm_grammar_parses_and_paints() {
    assert!(syntax::set_engine(wasmtime::Engine::default()));
    let lang = register();

    let source = r#"{"name": "cydonia", "count": 41, "ok": true}"#;
    let spans = Session::new()
        .highlight(lang, source)
        .expect("the wasm grammar loaded and parsed");

    assert!(!spans.is_empty());
    let kind_at = |needle: &str| {
        let at = source.find(needle).expect("needle is in the source");
        spans
            .iter()
            .find(|(range, _)| range.start <= at && at < range.end)
            .map(|(_, kind)| *kind)
    };
    assert_eq!(kind_at("\"name\""), Some(HighlightKind::Property));
    assert_eq!(kind_at("\"cydonia\""), Some(HighlightKind::String));
    assert_eq!(kind_at("41"), Some(HighlightKind::Number));
    assert_eq!(kind_at("true"), Some(HighlightKind::Boolean));
}

/// A second grammar loads into the same session's store — which means taking
/// the store back off the parser, loading, and putting it back. `css.wasm`
/// carries an external scanner, so this also covers a grammar whose
/// `scanner.c` was compiled in alongside `parser.c`.
#[test]
fn two_wasm_grammars_share_one_store() {
    assert!(syntax::set_engine(wasmtime::Engine::default()));
    let json = register();
    let css: &'static Lang = Box::leak(Box::new(Lang::new(
        "css",
        &["css"],
        Grammar::Wasm(Arc::from(CSS_WASM)),
        "(property_name) @property\n(integer_value) @number",
    )));
    registry::register(Entry {
        name: css.name,
        aliases: css.aliases,
        files: &["css"],
        lang: Some(css),
    });

    let mut session = Session::new();
    assert!(session.highlight(json, r#"{"a": 1}"#).is_some());
    let spans = session
        .highlight(css, ".a { width: 3px; }")
        .expect("css loaded into the same store");
    assert!(!spans.is_empty(), "the css grammar parsed nothing");
    // And the first grammar still parses after the store was moved about.
    assert!(session.highlight(json, r#"{"b": 2}"#).is_some());
}

/// Malformed bytes are a grammar that does not load, not a panic.
#[test]
fn bad_bytes_do_not_load() {
    assert!(syntax::set_engine(wasmtime::Engine::default()));
    let lang: &'static Lang = Box::leak(Box::new(Lang::new(
        "junk",
        &["junk"],
        Grammar::Wasm(Arc::from(&b"not a wasm module"[..])),
        QUERY,
    )));
    assert!(Session::new().highlight(lang, "{}").is_none());
}
