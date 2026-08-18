use markdown::*;

#[test]
fn text_accessor_covers_every_kind() {
    // The match in `Block::text` is exhaustive; this pins which side each
    // kind falls on, so adding a variant is a deliberate choice.
    let with_text = [
        BlockKind::Paragraph(Text::plain("a")),
        BlockKind::Heading {
            level: 1,
            text: Text::plain("a"),
        },
        BlockKind::Bullet(Text::plain("a")),
        BlockKind::Ordered {
            number: 1,
            text: Text::plain("a"),
        },
        BlockKind::Task {
            checked: false,
            text: Text::plain("a"),
        },
        BlockKind::Quote(Text::plain("a")),
    ];
    for kind in with_text {
        assert!(Block::new(kind).text().is_some());
    }

    let atomic = [
        BlockKind::Code {
            language: None,
            code: String::new(),
        },
        BlockKind::Image {
            url: String::new(),
            alt: String::new(),
        },
        BlockKind::Table {
            align: Vec::new(),
            header: Vec::new(),
            rows: Vec::new(),
        },
        BlockKind::Rule,
    ];
    for kind in atomic {
        assert!(Block::new(kind).text().is_none());
    }
}
