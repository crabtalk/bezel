use markdown::*;

use markdown::{parse::parse, serialize::serialize};

fn text_of(doc: &Doc, ix: usize) -> &Text {
    doc.blocks[ix].text().expect("block holds text")
}

#[test]
fn typing_at_the_end_of_a_mark_joins_it() {
    let mut text = parse("**ab**").blocks[0].text().unwrap().clone();
    text.insert(2, "c");
    assert_eq!(text.text, "abc");
    assert_eq!(text.marks[0].range, 0..3, "left-sticky: the tail is bold");
}

#[test]
fn typing_at_the_start_of_a_mark_stays_outside_it() {
    let mut text = parse("**ab**").blocks[0].text().unwrap().clone();
    text.insert(0, "x");
    assert_eq!(text.text, "xab");
    assert_eq!(text.marks[0].range, 1..3);
}

#[test]
fn removing_text_collapses_the_marks_over_it() {
    let mut text = parse("a**bc**d").blocks[0].text().unwrap().clone();
    text.remove(1..3);
    assert_eq!(text.text, "ad");
    assert!(
        text.marks
            .iter()
            .all(|span| span.range.end <= text.text.len())
    );
}

#[test]
fn toggling_twice_is_a_no_op() {
    let mut text = Text::plain("hello");
    let before = text.clone();
    text.toggle(0..5, Mark::Bold);
    assert!(text.covered_by(&(0..5), &Mark::Bold));
    text.toggle(0..5, Mark::Bold);
    assert_eq!(text, before);
}

#[test]
fn untoggling_the_middle_leaves_the_ends() {
    let mut text = Text::plain("abcde");
    text.toggle(0..5, Mark::Bold);
    text.toggle(1..4, Mark::Bold);
    let mut ranges: Vec<_> = text.marks.iter().map(|span| span.range.clone()).collect();
    ranges.sort_by_key(|range| range.start);
    assert_eq!(ranges, vec![0..1, 4..5]);
}

#[test]
fn abutting_marks_merge_so_they_serialize_as_one_run() {
    let mut text = Text::plain("abcd");
    text.toggle(0..2, Mark::Bold);
    text.toggle(2..4, Mark::Bold);
    assert_eq!(text.marks.len(), 1, "`**ab****cd**` is not one bold run");
    assert_eq!(text.marks[0].range, 0..4);
}

#[test]
fn enter_in_a_list_makes_another_item() {
    let mut doc = parse("- alpha");
    let new = doc.split(0, 3);
    assert_eq!(new, 1);
    assert!(matches!(doc.blocks[1].kind, BlockKind::Bullet(_)));
    assert_eq!(text_of(&doc, 0).text, "alp");
    assert_eq!(text_of(&doc, 1).text, "ha");
}

#[test]
fn enter_after_a_heading_gives_body_text() {
    let mut doc = parse("# Title");
    doc.split(0, 5);
    assert!(matches!(doc.blocks[1].kind, BlockKind::Paragraph(_)));
}

#[test]
fn backspace_walks_out_before_it_merges() {
    // Indented bullet: outdent, then unmarker, then merge.
    let mut doc = parse("- a\n    - b");
    assert_eq!(doc.blocks[1].indent, 1);

    doc.merge_back(1);
    assert_eq!(doc.blocks[1].indent, 0, "first press outdents");

    doc.merge_back(1);
    assert!(
        matches!(doc.blocks[1].kind, BlockKind::Paragraph(_)),
        "second press drops the marker"
    );

    let caret = doc.merge_back(1);
    assert_eq!(caret, Some(1), "third press merges after \"a\"");
    assert_eq!(doc.blocks.len(), 1);
    assert_eq!(text_of(&doc, 0).text, "ab");
}

#[test]
fn indenting_carries_the_children() {
    let mut doc = parse("- a\n- b\n    - c");
    assert!(doc.indent(1));
    assert_eq!(doc.blocks[1].indent, 1);
    assert_eq!(doc.blocks[2].indent, 2, "the child came along");
}

#[test]
fn a_block_cannot_indent_more_than_one_past_the_one_above() {
    let mut doc = parse("- a\n- b");
    assert!(doc.indent(1));
    assert!(
        !doc.indent(1),
        "two levels in one press would break the invariant"
    );
    assert!(!doc.indent(0), "the first block has nothing to nest under");
}

#[test]
fn shortcuts_cover_the_slash_menu_vocabulary() {
    assert_eq!(shortcut("# x"), Some((Shortcut::Heading(1), 2)));
    assert_eq!(shortcut("### x"), Some((Shortcut::Heading(3), 4)));
    assert_eq!(shortcut("- [x] x"), Some((Shortcut::Task(true), 6)));
    assert_eq!(shortcut("- [ ] x"), Some((Shortcut::Task(false), 6)));
    assert_eq!(shortcut("- x"), Some((Shortcut::Bullet, 2)));
    assert_eq!(shortcut("1. x"), Some((Shortcut::Ordered, 3)));
    assert_eq!(shortcut("> x"), Some((Shortcut::Quote, 2)));
    assert_eq!(shortcut("plain"), None);
    // A task marker must win over the bullet it starts with.
    assert_ne!(shortcut("- [ ] x").map(|hit| hit.0), Some(Shortcut::Bullet));
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as usize
    }
}

/// The property that matters: no sequence of edits can reach a document
/// the serializer cannot express. If this fails, the editor corrupts the
/// file on save.
#[test]
fn no_edit_sequence_escapes_the_round_trip() {
    const SEEDS: &[&str] = &[
        "# Title\n\nBody **bold** text",
        "- a\n- b\n    - c",
        "1. one\n2. two",
        "- [ ] task\n- [x] done",
        "> quoted\n\npara `code` tail",
        "para with [link](u) and ![img](i)",
    ];
    const WORDS: &[&str] = &["x", " ", "\n", "a b", "**", "#", "- ", "`", "|", "~~"];
    const MARKS: &[Mark] = &[Mark::Bold, Mark::Italic, Mark::Strike, Mark::Code];

    let mut rng = Rng(0xed17);
    for seed in SEEDS {
        for case in 0..20_000 {
            let mut doc = parse(seed);

            for _ in 0..6 {
                if doc.blocks.is_empty() {
                    break;
                }
                let ix = rng.next() % doc.blocks.len();
                let len = doc.blocks[ix].text().map_or(0, |text| text.text.len());
                let a = if len == 0 { 0 } else { rng.next() % (len + 1) };
                let b = if len == 0 { 0 } else { rng.next() % (len + 1) };

                match rng.next() % 7 {
                    0 => {
                        let word = WORDS[rng.next() % WORDS.len()];
                        doc.edit_text(ix, |text| text.insert(a, word));
                    }
                    1 => {
                        doc.edit_text(ix, |text| text.remove(a.min(b)..a.max(b)));
                    }
                    2 => {
                        let mark = MARKS[rng.next() % MARKS.len()].clone();
                        doc.edit_text(ix, |text| text.toggle(a.min(b)..a.max(b), mark));
                    }
                    3 => {
                        doc.split(ix, a);
                    }
                    4 => {
                        doc.merge_back(ix);
                    }
                    5 => {
                        doc.indent(ix);
                    }
                    _ => {
                        doc.outdent(ix);
                    }
                }

                // Offsets must stay inside the text they index.
                for block in &doc.blocks {
                    if let Some(text) = block.text() {
                        for span in &text.marks {
                            assert!(
                                span.range.end <= text.text.len()
                                    && text.text.is_char_boundary(span.range.start)
                                    && text.text.is_char_boundary(span.range.end),
                                "mark escaped its text: {span:?} in {:?}",
                                text.text
                            );
                        }
                    }
                }

                // And the document invariant the serializer leans on.
                for ix in 0..doc.blocks.len() {
                    assert!(
                        doc.blocks[ix].indent <= doc.ceiling(ix),
                        "block {ix} is deeper than anything above it can hold"
                    );
                }
            }

            doc.normalize();
            let text = serialize(&doc);
            assert_eq!(
                parse(&text),
                doc,
                "seed {seed:?} case {case}: edits left a document that does not \
                     survive its own serializer\n--- serialized ---\n{text:?}\n"
            );
        }
    }
}
