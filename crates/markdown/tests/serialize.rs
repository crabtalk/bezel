use markdown::*;

use markdown::parse::parse;

/// Markdown already in the form this serializer emits. These must survive
/// byte for byte — anything less means opening a note and saving it without
/// touching it would rewrite the file.
const CANONICAL: &[&str] = &[
    "Hello",
    "a\nb",
    "# Title",
    "###### Deep",
    "# Title\n\nBody",
    "**bold** and _italic_ and ~~struck~~",
    "**_x_**",
    "_**x**_",
    "`code` inline",
    "[link](https://example.com)",
    "- a\n- b",
    "- a\n    - b\n- c",
    "1. a\n2. b",
    "3. c\n4. d",
    "- [ ] todo\n- [x] done",
    "- [ ] nested\n    - [x] task",
    "- a\n    - [ ] b",
    "- [ ] a\n    - b",
    "+",
    "1.",
    "- [ ]\nafter",
    "- [ ] ",
    "> quoted",
    "```rust\nfn main() {}\n```",
    "```\nplain\n```",
    "---",
    "![alt](media://x)",
    "![](media://x)",
    "![alt](</My Notes/x.png>)",
    "[link](<a b>)",
    "[link](https://en.wikipedia.org/wiki/Rust_(programming_language))",
    "| a | b |\n| --- | ---: |\n| 1 | 2 |",
    "# Title\n\n- a\n- b\n\n> note\n\n```sh\nls\n```",
    "snake_case stays intact",
    "a #123 reference",
    "1 < 2 & 3 > 0",
    "- a\n\n    child paragraph",
    "> [!TIP]\n> GFM alerts",
    "> [!NOTE]\n> GFM alerts",
    "> [!WARNING]\n> GFM alerts",
    "> [!CAUTION]\n> GFM alerts",
    "> [!IMPORTANT]\n> GFM alerts",
    "> [!TIP]",
    "> [!TIP]\n> two\n> lines",
    // The first line of a quote is a line start like any other: what would
    // open a block there has to keep its backslash.
    "> \\- item",
    "> \\# hash",
    "> \\> angle",
    "> [!TIP]\n> \\- item",
    "> \\[!TIP\\]",
];

/// Markdown that legitimately gets rewritten — escaping added, nesting
/// flattened, alignment normalized. The guarantee here is only that the
/// rewrite settles after one pass.
const NON_CANONICAL: &[&str] = &[
    "star * and under _ alone",
    "a [bracket] in text",
    "trailing  spaces  kept",
    "> - quoted bullet",
    "- > bulleted quote",
    "> # heading in a quote",
    "- a\n            - overdeep",
    ">>> deep quote",
    "1) paren ordered",
    "* star bullet",
    "+ plus bullet",
    "Setext\n======",
    "| a |\n| :-: |\n| c |",
    "<div>raw html</div>",
    "text with <span>inline</span> html",
    "AT&T and &amp; entities",
    "1. a\n\n    ```\n    code\n    ```",
    "",
    "\n\n\n",
    "[!TIP](link)",
    "![!NOTE](image)",
    "[!WARNING]",
    "[!CAUTION] GFM alerts",
    "> [!IMPORTANT] GFM alerts",
    "> [!BOGUS]\n> not an alert",
    "> [!tip]\n> lowercase marker",
    "> [!TIP]\n> one\n>\n> two",
    "> [!NOTE]\n> outer\n> > [!TIP]\n> > inner",
];

#[test]
fn serializing_a_parse_is_a_fixed_point() {
    for source in CANONICAL.iter().chain(NON_CANONICAL) {
        let once = parse(source);
        let text = serialize(&once);
        let twice = parse(&text);
        assert_eq!(
            once, twice,
            "not a fixed point\n--- source ---\n{source}\n--- serialized ---\n{text}\n"
        );
    }
}

#[test]
fn canonical_input_survives_byte_for_byte() {
    for source in CANONICAL {
        let text = serialize(&parse(source));
        assert_eq!(text, *source, "canonical form drifted");
    }
}

#[test]
fn every_document_satisfies_the_indent_invariant() {
    // The serializer's indentation is only sound because of this.
    for source in CANONICAL.iter().chain(NON_CANONICAL) {
        let mut previous: Option<u8> = None;
        for block in &parse(source).blocks {
            let max = previous.map_or(0, |p| p + 1);
            assert!(block.indent <= max, "{source:?}: indent {}", block.indent);
            previous = Some(block.indent);
        }
    }
}

#[test]
fn mark_nesting_order_survives() {
    assert_eq!(serialize(&parse("**_x_**")), "**_x_**");
    assert_eq!(serialize(&parse("_**x**_")), "_**x**_");
}

#[test]
fn a_hash_reference_is_not_escaped() {
    // The bug that made desktop's `markdown.ts` necessary: a `#123` ref
    // escaped to `\#123` matches no reader.
    assert_eq!(serialize(&parse("see #123 now")), "see #123 now");
    assert_eq!(serialize(&parse("#123 at the start")), "#123 at the start");
}

#[test]
fn a_heading_in_body_text_is_escaped() {
    let doc = Doc {
        blocks: vec![Block::new(BlockKind::Paragraph(Text::plain(
            "# not a heading",
        )))],
    };
    let text = serialize(&doc);
    assert_eq!(text, "\\# not a heading");
    assert_eq!(parse(&text), doc);
}

#[test]
fn text_that_looks_like_a_list_is_escaped() {
    for body in ["- item", "1. item", "> quote", "+ item"] {
        let doc = Doc {
            blocks: vec![Block::new(BlockKind::Paragraph(Text::plain(body)))],
        };
        assert_eq!(parse(&serialize(&doc)), doc, "{body:?} did not survive");
    }
}

#[test]
fn code_containing_a_fence_gets_a_longer_one() {
    let doc = Doc {
        blocks: vec![Block::new(BlockKind::Code {
            language: None,
            code: Text::plain("```\nnested\n```"),
        })],
    };
    assert_eq!(parse(&serialize(&doc)), doc);
}

#[test]
fn a_destination_with_a_space_keeps_its_angles() {
    // A media path under a project whose name has a space in it. Written
    // bare, the destination ends at the space and the whole block comes back
    // as a line of text.
    let doc = Doc {
        blocks: vec![Block::new(BlockKind::Image {
            url: "/My Notes/shot.png".to_string(),
            alt: Text::plain("alt".to_string()),
            width: None,
        })],
    };
    assert_eq!(serialize(&doc), "![alt](</My Notes/shot.png>)");
    assert_eq!(parse(&serialize(&doc)), doc);
}

#[test]
fn a_destination_survives_whatever_it_holds() {
    for url in ["a b", "a(b", "a)b", "a\\b", "<a>", "a b(c"] {
        let doc = Doc {
            blocks: vec![Block::new(BlockKind::Paragraph(Text::link(url)))],
        };
        assert_eq!(parse(&serialize(&doc)), doc, "{url:?} did not survive");
    }
    // The one thing neither spelling holds: raw, it would end the
    // destination the same way a space does.
    let doc = Doc {
        blocks: vec![Block::new(BlockKind::Paragraph(Text::link("a\nb")))],
    };
    assert!(serialize(&doc).contains("<a%0Ab>"));
}

#[test]
fn an_empty_document_serializes_to_nothing() {
    assert_eq!(serialize(&Doc::default()), "");
    assert_eq!(parse(""), Doc::default());
}

/// Line fragments to shuffle into documents no one would think to write.
/// The point of the generative test is exactly the combinations a
/// hand-written corpus misses, so the awkward ones earn their place here.
const FRAGMENTS: &[&str] = &[
    "# h",
    "###### deep",
    "text",
    "- a",
    "- b",
    "1. a",
    "2. b",
    "9) paren",
    "> q",
    "```",
    "```rust",
    "    indented",
    "| a | b |",
    "| --- | ---: |",
    "| 1 | 2 |",
    "---",
    "***",
    "",
    "    - nested",
    "        deep cont",
    "- [ ] t",
    "- [x] d",
    "**bold** tail",
    "_it_ tail",
    "`code` tail",
    "[l](u) tail",
    "![](i)",
    "![alt](i)",
    "[l](<a b>)",
    "![](<a b.png>)",
    "[l](a(b)c)",
    "a*b",
    "a_b_c",
    "#123",
    "1 < 2 & 3",
    "AT&T",
    "\\# escaped",
    "~~s~~",
    "   ",
    "> - x",
    "- > x",
    "<div>",
    "a <span> b",
    "trailing \\",
    "**unclosed",
    "| ragged |",
    "setext",
    "======",
    "> [!TIP]",
    "> [!NOTE]",
    "> [!BOGUS]",
    "> \\[!TIP\\]",
];

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

#[test]
fn the_fixed_point_holds_for_generated_documents() {
    let mut rng = Rng(0x5eed);
    for case in 0..20_000 {
        let lines = 1 + rng.next() % 8;
        let source = (0..lines)
            .map(|_| FRAGMENTS[rng.next() % FRAGMENTS.len()])
            .collect::<Vec<_>>()
            .join("\n");

        let once = parse(&source);
        let text = serialize(&once);
        let twice = parse(&text);
        assert_eq!(
            once, twice,
            "case {case} is not a fixed point\n--- source ---\n{source}\n--- serialized ---\n{text}\n"
        );

        // And it must stay put: a third pass changes nothing either.
        assert_eq!(
            serialize(&twice),
            text,
            "case {case} drifted on a second pass"
        );
    }
}

#[test]
fn an_alert_is_a_block_kind_rather_than_text() {
    let doc = parse("> [!TIP]\n> body");
    assert_eq!(
        doc.blocks,
        vec![Block::new(BlockKind::Quote {
            kind: Some(QuoteKind::Tip),
            text: Text::plain("body"),
        })]
    );
    assert_eq!(serialize(&doc), "> [!TIP]\n> body");
}

#[test]
fn an_alert_marker_is_never_escaped() {
    // The bug this replaced: `> \[!TIP\]` is a quote holding two brackets on
    // every reader that knows what an alert is.
    for source in ["> [!TIP]\n> body", "> [!important]\n> body", "> [!NOTE]"] {
        assert!(!serialize(&parse(source)).contains('\\'), "{source:?}");
    }
}

#[test]
fn a_marker_that_names_no_alert_stays_text() {
    let doc = parse("> [!BOGUS]\n> body");
    assert_eq!(
        doc.blocks,
        vec![Block::new(BlockKind::Quote {
            kind: None,
            text: Text::plain("[!BOGUS]\nbody"),
        })]
    );
}

#[test]
fn normalize_keeps_an_alert_with_no_body() {
    // `Doc::normalize` drops an empty quote because `> ` cannot be written
    // down. `> [!TIP]` can, so it stays.
    let mut doc = parse("> [!TIP]");
    doc.normalize();
    assert_eq!(serialize(&doc), "> [!TIP]");

    let mut plain = Doc {
        blocks: vec![Block::new(BlockKind::Quote {
            kind: None,
            text: Text::default(),
        })],
    };
    plain.normalize();
    assert!(plain.blocks.is_empty());
}

#[test]
fn a_quotes_paragraphs_each_carry_the_alert() {
    // The flat model splits a blockquote's paragraphs into one block each, so
    // the kind rides on both and the rewrite is two alerts.
    let text = serialize(&parse("> [!TIP]\n> one\n>\n> two"));
    assert_eq!(text, "> [!TIP]\n> one\n\n> [!TIP]\n> two");
    assert_eq!(parse(&text), parse(&serialize(&parse(&text))));
}
