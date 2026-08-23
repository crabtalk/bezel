use markdown::*;

fn kinds(source: &str) -> Vec<(u8, BlockKind)> {
    parse(source)
        .blocks
        .into_iter()
        .map(|b| (b.indent, b.kind))
        .collect()
}

#[test]
fn paragraph_and_heading() {
    assert_eq!(
        kinds("# Title\n\nBody"),
        vec![
            (
                0,
                BlockKind::Heading {
                    level: 1,
                    text: Text::plain("Title")
                }
            ),
            (0, BlockKind::Paragraph(Text::plain("Body"))),
        ]
    );
}

#[test]
fn a_soft_break_is_a_newline_in_the_block() {
    // Whether it paints as a break or a space is the renderer's call; the
    // model keeps what was typed so the round trip is exact.
    assert_eq!(
        kinds("a\nb"),
        vec![(0, BlockKind::Paragraph(Text::plain("a\nb")))]
    );
}

#[test]
fn nested_bullets_become_indent_levels() {
    assert_eq!(
        kinds("- a\n    - b"),
        vec![
            (0, BlockKind::Bullet(Text::plain("a"))),
            (1, BlockKind::Bullet(Text::plain("b"))),
        ]
    );
}

#[test]
fn a_continuation_paragraph_is_a_child_of_its_bullet() {
    assert_eq!(
        kinds("- a\n\n    cont"),
        vec![
            (0, BlockKind::Bullet(Text::plain("a"))),
            (1, BlockKind::Paragraph(Text::plain("cont"))),
        ]
    );
}

#[test]
fn ordered_lists_keep_their_start() {
    assert_eq!(
        kinds("3. c\n4. d"),
        vec![
            (
                0,
                BlockKind::Ordered {
                    number: 3,
                    text: Text::plain("c")
                }
            ),
            (
                0,
                BlockKind::Ordered {
                    number: 4,
                    text: Text::plain("d")
                }
            ),
        ]
    );
}

#[test]
fn task_items_carry_their_checkbox() {
    assert_eq!(
        kinds("- [x] done\n- [ ] todo"),
        vec![
            (
                0,
                BlockKind::Task {
                    checked: true,
                    text: Text::plain("done")
                }
            ),
            (
                0,
                BlockKind::Task {
                    checked: false,
                    text: Text::plain("todo")
                }
            ),
        ]
    );
}

#[test]
fn a_lone_image_is_a_block() {
    assert_eq!(
        kinds("![alt](media://x)"),
        vec![(
            0,
            BlockKind::Image {
                url: "media://x".into(),
                alt: Text::plain("alt"),
                width: None
            }
        )]
    );
    // desktop's shape: no alt text at all.
    assert_eq!(
        kinds("![](media://x)"),
        vec![(
            0,
            BlockKind::Image {
                url: "media://x".into(),
                alt: Text::default(),
                width: None
            }
        )]
    );
}

#[test]
fn an_image_among_text_stays_inline() {
    let blocks = kinds("see ![alt](u) here");
    assert!(matches!(blocks[0].1, BlockKind::Paragraph(_)));
}

#[test]
fn mark_order_records_nesting() {
    // The whole reason marks are a list rather than flags: these two must
    // not collapse into the same value.
    let bold_outer = kinds("**_x_**");
    let italic_outer = kinds("_**x**_");
    assert_ne!(bold_outer, italic_outer);

    let BlockKind::Paragraph(text) = &bold_outer[0].1 else {
        panic!("expected a paragraph")
    };
    assert_eq!(text.text, "x");
    assert_eq!(
        text.marks
            .iter()
            .map(|m| m.mark.clone())
            .collect::<Vec<_>>(),
        vec![Mark::Bold, Mark::Italic]
    );
}

#[test]
fn code_blocks_keep_their_language_and_drop_the_fence_newline() {
    assert_eq!(
        kinds("```rust\nfn main() {}\n```"),
        vec![(
            0,
            BlockKind::Code {
                language: Some("rust".into()),
                code: Text::plain("fn main() {}")
            }
        )]
    );
}

#[test]
fn a_code_block_in_a_list_nests_under_its_bullet() {
    let blocks = kinds("- a\n\n    ```\n    x\n    ```");
    assert_eq!(blocks[0].0, 0);
    assert!(matches!(blocks[0].1, BlockKind::Bullet(_)));
    assert_eq!(blocks[1].0, 1);
    assert!(matches!(blocks[1].1, BlockKind::Code { .. }));
}

#[test]
fn tables_carry_alignment() {
    let blocks = kinds("| a | b |\n| :- | --: |\n| 1 | 2 |");
    let BlockKind::Table {
        align,
        header,
        rows,
    } = &blocks[0].1
    else {
        panic!("expected a table")
    };
    assert_eq!(align, &vec![Align::Left, Align::Right]);
    assert_eq!(header, &vec![Text::plain("a"), Text::plain("b")]);
    assert_eq!(rows, &vec![vec![Text::plain("1"), Text::plain("2")]]);
}

#[test]
fn the_indent_invariant_holds_for_awkward_nesting() {
    // `> - a` flattens; whatever it flattens to must still satisfy the
    // invariant the serializer relies on.
    for source in [
        "> - a",
        "- > a",
        "> # h",
        "- a\n        - deep",
        ">>> deep quote",
    ] {
        let doc = parse(source);
        let mut previous = None;
        for block in &doc.blocks {
            let max = previous.map_or(0, |p: u8| p + 1);
            assert!(
                block.indent <= max,
                "{source:?}: indent {} exceeds {max}",
                block.indent
            );
            previous = Some(block.indent);
        }
    }
}
