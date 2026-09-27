//! A document kept parsed across edits paints what a whole parse would.

mod fixture;

use std::ops::Range;
use syntax::{
    document::Document,
    lang::{Grammar, Lang},
    session::Session,
};
use theme::HighlightKind;

static RUST: Lang = Lang::new(
    "rust",
    &["rust"],
    Grammar::Native(tree_sitter_rust::LANGUAGE),
    tree_sitter_rust::HIGHLIGHTS_QUERY,
);

const SOURCE: &str = r#"/// A doc comment.
#[derive(Debug)]
pub struct Point { x: i32, y: f64 }

impl Point {
    pub fn new(x: i32) -> Self {
        let label = "origin"; // a comment
        println!("{label} {}", x + 1);
        Self { x, y: 0.5 }
    }
}
"#;

/// Adjacent spans of one kind as one, since the two painters may split a run
/// differently.
fn joined(spans: Vec<(Range<usize>, HighlightKind)>) -> Vec<(Range<usize>, HighlightKind)> {
    let mut out: Vec<(Range<usize>, HighlightKind)> = Vec::new();
    for (range, kind) in spans {
        match out.last_mut() {
            Some((last, held)) if *held == kind && last.end == range.start => last.end = range.end,
            _ => out.push((range, kind)),
        }
    }
    out
}

fn whole(lang: &'static Lang, source: &str) -> Vec<(Range<usize>, HighlightKind)> {
    joined(
        Session::new()
            .highlight(lang, source)
            .expect("the query compiles"),
    )
}

#[test]
fn a_fresh_document_paints_what_highlight_does() {
    let document = Document::with_lang(&RUST, SOURCE).expect("native, no injections");
    assert_eq!(joined(document.spans()), whole(&RUST, SOURCE));

    fixture::install();
    let css = ".a { color: red; } /* note */";
    let document = Document::new("css", css).expect("css resolves");
    assert_eq!(joined(document.spans()), whole(&fixture::CSS, css));
}

/// Typed a character at a time, the tree reparsed from the old one each time,
/// and the spans still those of a parse from nothing.
#[test]
fn edits_keep_the_spans_of_a_whole_parse() {
    let mut session = Session::new();
    let mut whole = |text: &str| joined(session.highlight(&RUST, text).unwrap());
    let mut document = Document::with_lang(&RUST, SOURCE).unwrap();
    let mut text = SOURCE.to_string();
    for ch in " * 2 // twice\n        let z = 'c';".chars() {
        let at = text.find(");\n        Self").unwrap();
        text.insert(at, ch);
        let changed = document.edit(at..at, at + ch.len_utf8(), &text);
        assert!(changed.iter().any(|range| range.contains(&at)));
        assert_eq!(joined(document.spans()), whole(&text), "after {ch:?}");
    }
    // A deletion across a string's closing quote reshapes the tree.
    let quote = text.find("\"origin\"").unwrap() + "\"origin".len();
    text.replace_range(quote..quote + 1, "");
    document.edit(quote..quote + 1, quote, &text);
    assert_eq!(joined(document.spans()), whole(&text));
}

/// Asked for a range, the spans are the whole text's, clipped to it.
#[test]
fn spans_in_a_range_are_the_whole_texts_clipped() {
    let document = Document::with_lang(&RUST, SOURCE).unwrap();
    let range = SOURCE.find("impl").unwrap()..SOURCE.find("Self {").unwrap();
    let clipped: Vec<_> = document
        .spans()
        .into_iter()
        .filter(|(span, _)| span.end > range.start && span.start < range.end)
        .map(|(span, kind)| (span.start.max(range.start)..span.end.min(range.end), kind))
        .collect();
    assert_eq!(joined(document.spans_in(range)), joined(clipped));
}

/// A language that injects another is not held as a document.
#[test]
fn a_language_with_injections_is_refused() {
    fixture::install();
    assert!(Document::new("html", "<p></p>").is_none());
}
