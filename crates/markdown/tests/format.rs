//! Marks applied by key and by typed delimiter. Both end in the same place —
//! a mark over a range — so both are checked through what they serialize to.

use markdown::{edit::inline_rule, parse::parse, serialize::serialize, *};

fn body(block: usize, offset: usize) -> Cursor {
    Cursor::new(block, Part::Body, offset)
}

fn written(mut doc: Doc) -> String {
    doc.normalize();
    serialize(&doc)
}

#[test]
fn toggling_a_selection_marks_it() {
    let mut doc = parse("plain words here");
    doc.toggle_mark(Selection::new(body(0, 6), body(0, 11)), Mark::Bold);
    assert_eq!(written(doc), "plain **words** here");
}

#[test]
fn toggling_a_fully_marked_selection_takes_it_off() {
    let mut doc = parse("plain **words** here");
    doc.toggle_mark(Selection::new(body(0, 6), body(0, 11)), Mark::Bold);
    assert_eq!(written(doc), "plain words here");
}

#[test]
fn a_partly_marked_selection_marks_the_rest() {
    let mut doc = parse("**one** two");
    doc.toggle_mark(Selection::new(body(0, 0), body(0, 7)), Mark::Bold);
    assert_eq!(
        written(doc),
        "**one two**",
        "cmd-B over bold and plain bolds the plain, it does not unbold"
    );
}

#[test]
fn a_mark_applies_to_every_block_a_selection_crosses() {
    let mut doc = parse("first\n\nsecond");
    doc.toggle_mark(Selection::new(body(0, 0), body(1, 6)), Mark::Bold);
    assert_eq!(written(doc), "**first**\n\n**second**");
}

#[test]
fn code_refuses_a_mark() {
    let mut doc = parse("```\nlet x = 1;\n```");
    let at = Cursor::new(0, Part::Code, 0);
    doc.toggle_mark(
        Selection::new(at, Cursor::new(0, Part::Code, 3)),
        Mark::Bold,
    );
    let BlockKind::Code { code, .. } = &doc.blocks[0].kind else {
        panic!("still code");
    };
    assert!(code.marks.is_empty(), "nothing in a fence is markup");
}

#[test]
fn spans_report_every_text_a_selection_touches() {
    let doc = parse("a\n\n| x | y |\n| --- | --- |\n| z | w |");
    let spans = doc.spans(Selection::new(
        body(0, 0),
        Cursor::new(1, Part::Cell { row: 1, column: 0 }, 1),
    ));
    assert_eq!(
        spans.len(),
        4,
        "the paragraph, two header cells, one body cell"
    );
    assert_eq!(spans[0].0, body(0, 0));
    assert_eq!(
        spans[1].0,
        Cursor::new(1, Part::Cell { row: 0, column: 0 }, 0)
    );
}

#[test]
fn a_closing_delimiter_collapses_the_run_it_closes() {
    let cases = [
        ("**bold**", 8, Mark::Bold, "bold"),
        ("~~gone~~", 8, Mark::Strike, "gone"),
        ("`code`", 6, Mark::Code, "code"),
        ("_slant_", 7, Mark::Italic, "slant"),
        ("*slant*", 7, Mark::Italic, "slant"),
    ];
    for (text, caret, mark, inner_text) in cases {
        let (open, inner, hit) = inline_rule(text, caret).unwrap_or_else(|| panic!("{text}"));
        assert_eq!(hit, mark, "{text}");
        assert_eq!(&text[inner], inner_text, "{text}");
        assert_eq!(open.start, 0, "{text}");
    }
}

#[test]
fn a_rule_that_would_vanish_does_not_fire() {
    // Emphasis cannot close against whitespace — the mark would be shrunk
    // straight back off by `normalize_marks`.
    assert!(inline_rule("**bold **", 9).is_none());
    assert!(inline_rule("** bold**", 9).is_none());
    // Nothing between the delimiters.
    assert!(inline_rule("****", 4).is_none());
    // And a lone delimiter closes nothing.
    assert!(inline_rule("just a * here", 8).is_none());
}

#[test]
fn an_intraword_underscore_is_not_emphasis() {
    assert!(
        inline_rule("snake_case_name", 11).is_none(),
        "or every identifier typed would go italic"
    );
    assert!(inline_rule("word _slant_", 12).is_some());
}

#[test]
fn bold_wins_over_italic_at_the_same_spot() {
    let (_, _, mark) = inline_rule("**x**", 5).unwrap();
    assert_eq!(mark, Mark::Bold, "`**` is bold, not two italics");
}
