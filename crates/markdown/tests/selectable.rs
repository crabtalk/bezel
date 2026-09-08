//! What a pointer selection copies out as.
//!
//! Not the editor's copy, which is `slice` then `serialize` and keeps the
//! markup (see `clipboard.rs`). This is the plain text a reader dragged over —
//! what a paste into anything else should read like.

use markdown::{parse::parse, selectable::copied, *};

fn body(block: usize, offset: usize) -> Cursor {
    Cursor::new(block, Part::Body, offset)
}

const DOC: &str = "# Title\n\nA paragraph with **bold** in it.\n\n- first\n- second";

#[test]
fn a_selection_inside_one_block_is_the_text_between_its_ends() {
    let doc = parse(DOC);
    let selection = Selection::new(body(1, 2), body(1, 11));
    assert_eq!(copied(&doc, selection), "paragraph");
}

#[test]
fn crossing_blocks_puts_a_newline_where_each_one_ended() {
    let doc = parse(DOC);
    let selection = Selection::new(body(2, 0), body(3, 6));
    assert_eq!(copied(&doc, selection), "first\nsecond");
}

#[test]
fn what_comes_out_is_text_rather_than_markup() {
    // The paragraph holds `**bold**`; a reader dragging over it selected the
    // word, not the asterisks — which is the whole difference from the
    // editor's own copy.
    let doc = parse(DOC);
    let whole = Selection::new(body(0, 0), body(1, 30));
    let text = copied(&doc, whole);
    assert!(
        !text.contains('*'),
        "marks are not part of the text: {text:?}"
    );
    assert!(text.starts_with("Title\n"), "{text:?}");
}

#[test]
fn a_press_that_never_dragged_copies_nothing() {
    let doc = parse(DOC);
    assert_eq!(copied(&doc, Selection::at(body(1, 4))), "");
}
