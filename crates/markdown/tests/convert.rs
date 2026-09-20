//! `From` at the model's edges: source in, document out, and back.

use markdown::{
    Block, BlockKind, Cursor, Doc, Marks, ParsedDoc, Part, Selection, Text, parse, parse_with,
    serialize,
};

const SOURCE: &str = "# Title\n\nA paragraph.\n";

#[test]
fn source_converts_to_a_doc_and_back() {
    let doc: Doc = SOURCE.into();
    assert_eq!(doc, parse(SOURCE));

    let out: String = (&doc).into();
    assert_eq!(out, serialize(&doc));
}

#[test]
fn source_converts_to_a_parse_with_ranges() {
    let parsed: ParsedDoc = SOURCE.into();
    assert_eq!(parsed.doc, parse(SOURCE));
    assert_eq!(parsed.block_ranges.len(), parsed.doc.blocks.len());
}

/// The neutral reading of a `&str`: prose, not a link.
#[test]
fn text_converts_unmarked() {
    let text: Text = "plain".into();
    assert_eq!(text, Text::plain("plain"));
    assert!(text.marks.is_empty());

    let owned: Text = String::from("plain").into();
    assert_eq!(owned, text);
}

#[test]
fn a_kind_converts_to_a_block_at_the_left_margin() {
    let block: Block = BlockKind::Paragraph(Text::plain("hi")).into();
    assert_eq!(block.indent, 0);
    assert_eq!(block, Block::new(BlockKind::Paragraph(Text::plain("hi"))));
}

#[test]
fn a_cursor_converts_to_a_collapsed_selection() {
    let cursor = Cursor::new(0, Part::Body, 3);
    let selection: Selection = cursor.into();
    assert_eq!(selection, Selection::at(cursor));
    assert!(selection.is_collapsed());
}

/// The marks a document is read with ride in beside the source, so an app with
/// its own is not pushed back to `parse_with` by reaching for `.into()`.
#[test]
fn source_and_marks_convert_to_a_doc() {
    let marks = Marks::new().with("highlight", "==");
    let source = "a ==lit== word";

    let doc: Doc = (source, &marks).into();
    assert_eq!(doc, parse_with(source, &marks));
    assert_ne!(doc, parse(source));
}
