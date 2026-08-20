//! Copy is `slice` then `serialize`; paste is `parse` then `splice`. The one
//! property worth holding is that the pair is lossless for whole blocks: what
//! comes off the clipboard is what goes back on.

use markdown::{parse::parse, serialize::serialize, *};

fn body(block: usize, offset: usize) -> Cursor {
    Cursor::new(block, Part::Body, offset)
}

fn copied(doc: &Doc, selection: Selection) -> String {
    written(doc.slice(selection))
}

/// What the editor actually writes: normalized, then serialized.
fn written(mut doc: Doc) -> String {
    doc.normalize();
    serialize(&doc)
}

#[test]
fn a_slice_within_one_block_is_the_text_between_the_ends() {
    let doc = parse("the **quick** brown fox");
    assert_eq!(
        copied(&doc, Selection::new(body(0, 4), body(0, 9))),
        "**quick**"
    );
}

#[test]
fn a_slice_across_blocks_keeps_every_kind_it_crossed() {
    let doc = parse("# Title\n\n- one\n- two\n\ntail");
    let source = copied(&doc, Selection::new(body(0, 2), body(2, 3)));
    assert_eq!(source, "# tle\n\n- one\n- two");
}

#[test]
fn a_slice_starts_at_the_left_margin_however_deep_it_was_cut_from() {
    let doc = parse("- a\n    - b\n        - c");
    let source = copied(&doc, Selection::new(body(1, 0), body(2, 1)));
    assert_eq!(
        source, "- b\n    - c",
        "the outermost block of a slice is its new root"
    );
}

#[test]
fn a_lone_paragraph_pastes_inline_with_its_marks() {
    let mut doc = parse("before  after");
    let head = doc.splice(Selection::at(body(0, 7)), parse("**bold**"));
    assert_eq!(written(doc.clone()), "before **bold** after");
    assert_eq!(head, body(0, 11));
    assert_eq!(doc.blocks.len(), 1, "no new block for a sentence");
}

#[test]
fn more_than_a_paragraph_pastes_as_blocks() {
    let mut doc = parse("start end");
    doc.splice(Selection::at(body(0, 6)), parse("# Head\n\n- item"));
    assert_eq!(written(doc), "start\n\n# Head\n\n- item\n\nend");
}

#[test]
fn pasting_into_an_empty_document_leaves_no_blank_block() {
    let mut doc = parse("");
    doc.splice(Selection::default(), parse("# Head\n\nbody"));
    assert_eq!(written(doc), "# Head\n\nbody");
}

#[test]
fn a_pasted_selection_replaces_what_it_lands_on() {
    let mut doc = parse("keep this drop that\n\nsecond");
    doc.splice(Selection::new(body(0, 10), body(1, 6)), parse("X"));
    assert_eq!(written(doc), "keep this X");
}

/// The round trip that matters: copy a range, paste it into an empty document,
/// and the markdown is the same. If this drifts, a cut/paste loses work.
#[test]
fn copy_then_paste_reproduces_the_slice() {
    const SOURCES: &[&str] = &[
        "# Title\n\nBody **bold** text\n\n- a\n- b",
        "- [ ] task\n- [x] done\n\n> quote",
        "para\n\n```rust\nfn main() {}\n```",
        "| a | b |\n| --- | --- |\n| c | d |",
        "one\n\n---\n\ntwo",
    ];

    for source in SOURCES {
        let doc = parse(source);
        let all = Selection::all(&doc);
        let cut = copied(&doc, all);

        let mut into = Doc::default();
        into.splice(Selection::default(), parse(&cut));
        assert_eq!(
            written(into),
            cut,
            "pasting a copy of {source:?} did not reproduce it"
        );
    }
}
