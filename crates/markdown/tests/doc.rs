use markdown::*;

/// Which part each kind exposes, so adding a variant is a deliberate choice
/// rather than a block a caret silently cannot enter.
#[test]
fn every_kind_declares_the_parts_a_caret_can_enter() {
    let cases = [
        (BlockKind::Paragraph(Text::plain("a")), vec![Part::Body]),
        (
            BlockKind::Heading {
                level: 1,
                text: Text::plain("a"),
            },
            vec![Part::Body],
        ),
        (BlockKind::Bullet(Text::plain("a")), vec![Part::Body]),
        (
            BlockKind::Ordered {
                number: 1,
                text: Text::plain("a"),
            },
            vec![Part::Body],
        ),
        (
            BlockKind::Task {
                checked: false,
                text: Text::plain("a"),
            },
            vec![Part::Body],
        ),
        (BlockKind::Quote(Text::plain("a")), vec![Part::Body]),
        (
            BlockKind::Code {
                language: None,
                code: Text::default(),
            },
            vec![Part::Code],
        ),
        (
            BlockKind::Image {
                url: String::new(),
                alt: String::new(),
            },
            Vec::new(),
        ),
        (BlockKind::Rule, Vec::new()),
        (
            BlockKind::Table {
                align: Vec::new(),
                header: vec![Text::plain("h")],
                rows: vec![vec![Text::plain("r")]],
            },
            vec![
                Part::Cell { row: 0, column: 0 },
                Part::Cell { row: 1, column: 0 },
            ],
        ),
    ];

    for (kind, parts) in cases {
        let block = Block::new(kind);
        assert_eq!(block.parts(), parts, "{:?}", block.kind);
        // Every declared part has to resolve, or a caret can reach a position
        // no edit can then reach.
        for part in block.parts() {
            assert!(block.text_at(part).is_some(), "{:?} {part:?}", block.kind);
        }
    }
}

#[test]
fn a_table_indexes_its_header_as_row_zero() {
    let block = Block::new(BlockKind::Table {
        align: Vec::new(),
        header: vec![Text::plain("head")],
        rows: vec![vec![Text::plain("first")], vec![Text::plain("second")]],
    });
    let text = |row, column| {
        block
            .text_at(Part::Cell { row, column })
            .map(|text| text.text.as_str())
    };
    assert_eq!(text(0, 0), Some("head"));
    assert_eq!(text(1, 0), Some("first"));
    assert_eq!(text(2, 0), Some("second"));
    assert_eq!(text(3, 0), None);
}
