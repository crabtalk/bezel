//! The caret's trip between a document and its markdown, and the colour the
//! source view paints on the way.

use markdown::{BlockKind, Cursor, Doc, Part, parse, parse_at, serialize, serialize_at};
use theme::HighlightKind;

/// Every caret position in `source`, block by block and part by part.
fn carets(doc: &Doc) -> Vec<Cursor> {
    doc.blocks
        .iter()
        .enumerate()
        .flat_map(|(ix, block)| {
            block.parts().into_iter().flat_map(move |part| {
                let len = block.text_at(part).map_or(0, |text| text.text.len());
                (0..=len)
                    .filter(move |offset| {
                        block
                            .text_at(part)
                            .is_some_and(|text| text.text.is_char_boundary(*offset))
                    })
                    .map(move |offset| Cursor::new(ix, part, offset))
            })
        })
        .collect()
}

const DOCUMENTS: [&str; 7] = [
    "# Title\n\nA paragraph with **bold** and `code` in it.",
    "- one\n- two\n    - nested",
    "1. first\n2. second",
    "> quoted text\n\nafter",
    "```rs\nfn main() {}\n```",
    "| a | b |\n| --- | --- |\n| 1 | 2 |",
    "A [link](https://example.com) and ![alt](https://example.com/i.png)",
];

#[test]
fn a_caret_comes_back_from_the_source_where_it_went_in() {
    for source in DOCUMENTS {
        let doc = parse(source);
        for at in carets(&doc) {
            let (written, offset) = serialize_at(&doc, at);
            assert_eq!(written, serialize(&doc), "the source is the plain one");
            let (back, landed) = parse_at(&written, offset);
            assert_eq!(back, doc, "the document survives the trip: {source:?}");
            assert_eq!(landed, at, "and so does the caret, in {written:?}");
        }
    }
}

#[test]
fn a_caret_the_source_cannot_hold_lands_at_the_start() {
    // Between a heading's `#` and its space: a caret there is a position in
    // the markup rather than in the document, and no block owns it.
    let (doc, at) = parse_at("# Title", 1);
    assert_eq!(doc, parse("# Title"));
    assert_eq!(at, Cursor::new(0, Part::Body, 0));
}

#[test]
fn source_colour_is_disjoint_and_in_order() {
    let source = DOCUMENTS.join("\n\n");
    let spans = markdown::source_spans(&source);
    let mut previous = 0;
    for (range, _) in &spans {
        assert!(range.start >= previous, "spans overlap or run backwards");
        assert!(range.end <= source.len(), "a span runs past the source");
        previous = range.end;
    }
    assert!(!spans.is_empty(), "something in all of that is coloured");
}

#[test]
fn a_heading_and_a_fence_are_coloured_as_themselves() {
    let source = "# Title\n\n`code`";
    let spans = markdown::source_spans(source);
    let kind_at = |at: usize| {
        spans
            .iter()
            .find(|(range, _)| range.contains(&at))
            .map(|(_, kind)| *kind)
    };
    assert_eq!(kind_at(0), Some(HighlightKind::Keyword), "the `#`");
    assert_eq!(kind_at(3), Some(HighlightKind::Keyword), "and its text");
    assert_eq!(kind_at(10), Some(HighlightKind::String), "the inline code");
}

#[test]
fn the_source_view_holds_the_whole_document() {
    let doc = parse(DOCUMENTS[0]);
    let (source, _) = serialize_at(&doc, Cursor::default());
    let back = parse(&source);
    assert_eq!(back, doc);
    assert!(matches!(back.blocks[0].kind, BlockKind::Heading { .. }));
}
