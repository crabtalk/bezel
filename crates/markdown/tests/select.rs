use markdown::{parse::parse, serialize::serialize, *};

fn body(block: usize, offset: usize) -> Cursor {
    Cursor::new(block, Part::Body, offset)
}

fn text_of(doc: &Doc, ix: usize) -> &str {
    &doc.blocks[ix]
        .text_at(Part::Body)
        .expect("block holds a body")
        .text
}

#[test]
fn a_cursor_orders_by_block_then_part_then_offset() {
    assert!(body(0, 9) < body(1, 0));
    assert!(body(1, 2) < body(1, 5));
    assert!(Cursor::new(0, Part::Body, 99) < Cursor::new(0, Part::Code, 0));
    assert!(
        Cursor::new(0, Part::Cell { row: 0, column: 3 }, 0)
            < Cursor::new(0, Part::Cell { row: 1, column: 0 }, 0)
    );
}

#[test]
fn ordered_is_independent_of_which_end_moved() {
    let forward = Selection::new(body(0, 1), body(2, 3));
    let backward = Selection::new(body(2, 3), body(0, 1));
    assert_eq!(forward.ordered(), backward.ordered());
    assert_eq!(forward.ordered(), (body(0, 1), body(2, 3)));
}

#[test]
fn the_caret_reaches_into_a_code_block() {
    let doc = parse("para\n\n```rust\nfn main() {}\n```");
    let into = body(0, 4).right(&doc);
    assert_eq!(into, Cursor::new(1, Part::Code, 0));
    assert_eq!(into.end(&doc).offset, "fn main() {}".len());
}

#[test]
fn the_caret_walks_the_cells_of_a_table() {
    let doc = parse("| a | b |\n| --- | --- |\n| c | d |");
    let first = Cursor::new(0, Part::Cell { row: 0, column: 0 }, 1);
    assert_eq!(
        first.right(&doc),
        Cursor::new(0, Part::Cell { row: 0, column: 1 }, 0),
        "the end of a cell steps into the next one"
    );
    assert_eq!(
        first.down(&doc),
        Cursor::new(0, Part::Cell { row: 1, column: 0 }, 1),
        "down stays in the column"
    );
}

#[test]
fn a_clamp_rescues_a_position_whose_part_is_gone() {
    let doc = parse("just a paragraph");
    let stale = Cursor::new(0, Part::Cell { row: 4, column: 2 }, 7);
    assert_eq!(stale.clamp(&doc), body(0, 7), "falls back to the body");
}

#[test]
fn replacing_within_one_block_keeps_the_marks_around_it() {
    let mut doc = parse("a **bold** tail");
    let head = doc.replace(Selection::new(body(0, 0), body(0, 1)), Text::plain("X"));
    assert_eq!(text_of(&doc, 0), "X bold tail");
    assert_eq!(head, body(0, 1));
    assert_eq!(
        serialize(&doc),
        "X **bold** tail",
        "the mark still covers it"
    );
}

#[test]
fn replacing_across_blocks_leaves_the_head_kind() {
    let mut doc = parse("# Title\n\nbody text\n\ntail");
    let head = doc.replace(Selection::new(body(0, 2), body(1, 4)), Text::default());
    assert_eq!(doc.blocks.len(), 2, "the two blocks became one");
    assert!(
        matches!(doc.blocks[0].kind, BlockKind::Heading { .. }),
        "the head keeps its kind"
    );
    assert_eq!(
        text_of(&doc, 0),
        "Ti text",
        "and takes the tail's remainder"
    );
    assert_eq!(
        text_of(&doc, 1),
        "tail",
        "the block past the end is untouched"
    );
    assert_eq!(head, body(0, 2));
}

#[test]
fn a_selection_that_swallows_a_rule_takes_it_whole() {
    let mut doc = parse("before\n\n---\n\nafter");
    doc.replace(Selection::new(body(0, 6), body(2, 0)), Text::default());
    assert_eq!(doc.blocks.len(), 1);
    assert_eq!(text_of(&doc, 0), "beforeafter");
}

#[test]
fn a_selection_leaving_a_table_takes_the_whole_table() {
    let mut doc = parse("| a | b |\n| --- | --- |\n| c | d |\n\nafter");
    let from = Cursor::new(0, Part::Cell { row: 0, column: 0 }, 0);
    doc.replace(Selection::new(from, body(1, 5)), Text::default());
    assert!(
        !doc.blocks
            .iter()
            .any(|block| matches!(block.kind, BlockKind::Table { .. })),
        "half a table has no shape worth keeping"
    );
}

#[test]
fn clearing_cells_within_one_table_keeps_its_shape() {
    let mut doc = parse("| a | b |\n| --- | --- |\n| c | d |");
    doc.replace(
        Selection::new(
            Cursor::new(0, Part::Cell { row: 0, column: 0 }, 0),
            Cursor::new(0, Part::Cell { row: 1, column: 0 }, 1),
        ),
        Text::default(),
    );
    let BlockKind::Table { header, rows, .. } = &doc.blocks[0].kind else {
        panic!("the table survives");
    };
    assert_eq!(header.len(), 2);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][1].text, "d", "the cell past the end is untouched");
}

#[test]
fn code_refuses_the_marks_a_paste_carries_in() {
    let mut doc = parse("```\ncode\n```");
    let at = Cursor::new(0, Part::Code, 4);
    let mut bold = Text::plain("X");
    bold.marks.push(MarkSpan {
        range: 0..1,
        mark: Mark::Bold,
    });
    doc.replace(Selection::at(at), bold);
    let BlockKind::Code { code, .. } = &doc.blocks[0].kind else {
        panic!("still a code block");
    };
    assert_eq!(code.text, "codeX");
    assert!(code.marks.is_empty(), "code is literal");
}

#[test]
fn word_motion_crosses_the_run_and_the_space_before_it() {
    let doc = parse("alpha beta gamma");
    assert_eq!(body(0, 16).word_left(&doc), body(0, 11));
    assert_eq!(body(0, 11).word_left(&doc), body(0, 6));
    assert_eq!(body(0, 0).word_right(&doc), body(0, 5));
    assert_eq!(body(0, 5).word_right(&doc), body(0, 10));
}

#[test]
fn select_all_spans_the_document() {
    let doc = parse("# Title\n\nbody");
    let all = Selection::all(&doc);
    assert_eq!(all.ordered(), (body(0, 0), body(1, 4)));
}

/// The guarantee, over the new primitive: no selection replacement can leave a
/// document the serializer cannot write back.
#[test]
fn no_replacement_escapes_the_round_trip() {
    const SEEDS: &[&str] = &[
        "# Title\n\nBody **bold** text\n\n- a\n- b",
        "| a | b |\n| --- | --- |\n| c | d |\n\ntail",
        "para\n\n```rust\nfn main() {}\n```\n\n> quote",
        "- [ ] task\n\n---\n\n![alt](u)",
    ];
    const WORDS: &[&str] = &["x", " ", "\n", "a b", "**", "#", "- ", "`", "|", "~~"];

    let mut rng = 0x5eedu64;
    let mut next = move || {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        (rng >> 33) as usize
    };

    for seed in SEEDS {
        for case in 0..5_000 {
            let mut doc = parse(seed);
            for _ in 0..4 {
                if doc.blocks.is_empty() {
                    break;
                }
                let pick = |doc: &Doc, n: usize| {
                    let ix = n % doc.blocks.len();
                    let parts = doc.blocks[ix].parts();
                    let part = parts
                        .get(n % parts.len().max(1))
                        .copied()
                        .unwrap_or_default();
                    let len = Cursor::new(ix, part, 0).len_in(doc).unwrap_or(0);
                    Cursor::new(ix, part, if len == 0 { 0 } else { n % (len + 1) })
                };
                let selection = Selection::new(pick(&doc, next()), pick(&doc, next()));
                doc.replace(selection, Text::plain(WORDS[next() % WORDS.len()]));
            }

            doc.normalize();
            let written = serialize(&doc);
            assert_eq!(
                parse(&written),
                doc,
                "seed {seed:?} case {case}: a replacement left a document that \
                 does not survive its own serializer\n--- serialized ---\n{written:?}\n"
            );
        }
    }
}
