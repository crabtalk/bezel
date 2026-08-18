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
    "> quoted",
    "```rust\nfn main() {}\n```",
    "```\nplain\n```",
    "---",
    "![alt](media://x)",
    "![](media://x)",
    "| a | b |\n| --- | ---: |\n| 1 | 2 |",
    "# Title\n\n- a\n- b\n\n> note\n\n```sh\nls\n```",
    "snake_case stays intact",
    "a #123 reference",
    "1 < 2 & 3 > 0",
    "- a\n\n    child paragraph",
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
    "- [ ] nested\n    - [x] task",
    "1. a\n\n    ```\n    code\n    ```",
    "",
    "\n\n\n",
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
            code: "```\nnested\n```".into(),
        })],
    };
    assert_eq!(parse(&serialize(&doc)), doc);
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
