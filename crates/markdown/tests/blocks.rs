//! Moving, duplicating and deleting a block. Each carries the blocks nested
//! under it, which on a flat list is a scan rather than a tree walk — and is
//! the whole argument for the flat list.

use markdown::{parse::parse, serialize::serialize, *};

fn written(mut doc: Doc) -> String {
    doc.normalize();
    serialize(&doc)
}

#[test]
fn a_subtree_is_the_block_and_everything_deeper() {
    let doc = parse("- a\n    - b\n    - c\n- d");
    assert_eq!(doc.subtree(0), 0..3, "the bullet and both children");
    assert_eq!(doc.subtree(1), 1..2, "a leaf is only itself");
    assert_eq!(doc.subtree(3), 3..4);
}

#[test]
fn moving_down_clears_the_whole_neighbour() {
    let doc_source = "- first\n- second\n    - child\n- third";
    let mut doc = parse(doc_source);
    doc.move_block(0, 1);
    assert_eq!(
        written(doc),
        "- second\n    - child\n- first\n- third",
        "one press steps over the neighbour and its children, not one row"
    );
}

#[test]
fn moving_up_carries_the_children() {
    let mut doc = parse("- first\n- second\n    - child");
    doc.move_block(1, -1);
    assert_eq!(written(doc), "- second\n    - child\n- first");
}

#[test]
fn a_block_at_the_edge_does_not_move() {
    let mut doc = parse("- a\n- b");
    assert_eq!(doc.move_block(0, -1), None, "nothing above the first");
    assert_eq!(doc.move_block(1, 1), None, "nothing below the last");
    assert_eq!(written(doc), "- a\n- b", "and the document is untouched");
}

#[test]
fn duplicating_copies_the_children_too() {
    let mut doc = parse("- a\n    - child\n- b");
    let copy = doc.duplicate(0);
    assert_eq!(copy, Some(2));
    assert_eq!(written(doc), "- a\n    - child\n- a\n    - child\n- b");
}

#[test]
fn deleting_takes_the_children_with_it() {
    let mut doc = parse("- a\n    - child\n- b");
    doc.remove_block(0);
    assert_eq!(written(doc), "- b");
}

#[test]
fn a_move_leaves_the_indent_invariant_intact() {
    // Moving a shallow block above a deep one could strand it a level too
    // deep; `repair` is what stops that reaching the serializer.
    let mut doc = parse("- a\n    - b\n        - c\n- d");
    doc.move_block(3, -1);
    for ix in 0..doc.blocks.len() {
        assert!(
            doc.blocks[ix].indent <= doc.ceiling(ix),
            "block {ix} is deeper than anything above it can hold"
        );
    }
    let text = written(doc.clone());
    doc.normalize();
    assert_eq!(parse(&text), doc, "and it still survives its serializer");
}

#[test]
fn setting_a_kind_carries_the_text_across() {
    let mut doc = parse("- a **bold** item");
    doc.set_kind(
        0,
        BlockKind::Heading {
            level: 2,
            text: Text::default(),
        },
    );
    assert_eq!(written(doc), "## a **bold** item");
}

#[test]
fn setting_a_kind_to_code_drops_the_marks_it_cannot_hold() {
    let mut doc = parse("some **bold** text");
    doc.set_kind(
        0,
        BlockKind::Code {
            language: None,
            code: Text::default(),
        },
    );
    let BlockKind::Code { code, .. } = &doc.blocks[0].kind else {
        panic!("now a code block");
    };
    assert_eq!(code.text, "some bold text");
    assert!(code.marks.is_empty(), "code is literal");
}
