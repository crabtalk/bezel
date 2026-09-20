//! The source range each block was parsed from — what an app splices with to
//! keep the bytes it did not edit.

use markdown::{BlockKind, ParsedDoc, Text, parse_ranges, serialize};

/// Bullets with an extra space, a setext heading, a fence: a document written
/// the way a person writes one, and nothing the serializer would leave alone.
const SOURCE: &str = "Intro
=====

A paragraph.

*  first
*  second

```rs
fn main() {}
```

Tail.
";

#[test]
fn a_range_for_every_block_partitions_the_source() {
    let ParsedDoc { doc, block_ranges } = parse_ranges(SOURCE);

    assert_eq!(block_ranges.len(), doc.blocks.len(), "one per block");
    assert_eq!(block_ranges[0].start, 0, "the first takes the head");
    assert_eq!(
        block_ranges.last().unwrap().end,
        SOURCE.len(),
        "the last takes the tail"
    );
    for pair in block_ranges.windows(2) {
        assert_eq!(pair[0].end, pair[1].start, "no gap and no overlap");
    }
    for range in &block_ranges {
        assert!(range.start <= range.end && range.end <= SOURCE.len());
        assert!(
            SOURCE.is_char_boundary(range.start) && SOURCE.is_char_boundary(range.end),
            "a range a caller can slice with"
        );
    }
}

#[test]
fn a_range_holds_the_syntax_its_block_was_written_in() {
    let ParsedDoc { doc, block_ranges } = parse_ranges(SOURCE);

    let slice = |kind: fn(&BlockKind) -> bool| {
        let ix = doc.blocks.iter().position(|block| kind(&block.kind));
        &SOURCE[block_ranges[ix.expect("block not found")].clone()]
    };

    assert!(
        slice(|kind| matches!(kind, BlockKind::Heading { .. })).contains("====="),
        "the heading kept the setext underline it was spelled with"
    );
    assert!(
        slice(|kind| matches!(kind, BlockKind::Bullet(_))).starts_with("*  "),
        "and the bullet kept its star and its spacing"
    );
    assert!(
        slice(|kind| matches!(kind, BlockKind::Code { .. })).contains("```rs"),
        "and the fence its info string"
    );
}

#[test]
fn splicing_an_unedited_block_back_is_byte_for_byte() {
    let ParsedDoc { doc, block_ranges } = parse_ranges(SOURCE);

    let whole: String = block_ranges
        .iter()
        .map(|range| &SOURCE[range.clone()])
        .collect();
    assert_eq!(whole, SOURCE, "the ranges put the document back together");

    // One paragraph rewritten, every other block spliced from the original
    // bytes — the save an app does to keep a one-word edit a one-word diff.
    let edited = doc
        .blocks
        .iter()
        .position(|block| matches!(&block.kind, BlockKind::Paragraph(text) if text.text == "A paragraph."))
        .expect("the paragraph is in there");
    let spliced: String = block_ranges
        .iter()
        .enumerate()
        .map(|(ix, range)| match ix == edited {
            true => "A rewritten paragraph.\n\n".to_string(),
            false => SOURCE[range.clone()].to_string(),
        })
        .collect();

    assert!(
        spliced.contains("Intro\n=====") && spliced.contains("*  first"),
        "the blocks nobody touched kept their bytes"
    );
    assert!(spliced.contains("A rewritten paragraph."));

    // Which is the point: the serializer writes the document in canonical
    // form, and that is a diff over every block rather than over one.
    let mut canonical = doc.clone();
    canonical.blocks[edited].kind = BlockKind::Paragraph(Text::plain("A rewritten paragraph."));
    let canonical = serialize(&canonical);
    assert!(
        !canonical.contains("*  first") && !canonical.contains("====="),
        "the serializer rewrote the blocks the splice left alone"
    );
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

const FRAGMENTS: &[&str] = &[
    "plain text",
    "# heading",
    "Setext\n------",
    "*  star bullet",
    "- [ ] a task",
    "- [x] a done task",
    "1. ordered",
    "9) ordered too",
    "> quoted",
    "> [!TIP]\n> an alert",
    "```rs\nfn main() {}\n```",
    "| a | b |\n| --- | --- |\n| 1 | 2 |",
    "---",
    "![alt](cover.png)",
    "<https://bezel.gallery>",
    "- ![alt](cover.png)",
    "",
    "    indented code",
    "nested\n\n  - under it",
];

/// The invariants a splice rests on, over documents nobody wrote by hand.
#[test]
fn the_partition_holds_for_generated_documents() {
    let mut rng = Rng(0x5eed);
    for case in 0..5_000 {
        let parts = 1 + rng.next() % 6;
        let source = (0..parts)
            .map(|_| FRAGMENTS[rng.next() % FRAGMENTS.len()])
            .collect::<Vec<_>>()
            .join("\n\n");

        let ParsedDoc { doc, block_ranges } = parse_ranges(&source);
        let report = format!("case {case}\n--- source ---\n{source}\n");

        assert_eq!(block_ranges.len(), doc.blocks.len(), "{report}");
        if block_ranges.is_empty() {
            continue;
        }
        assert_eq!(block_ranges[0].start, 0, "{report}");
        assert_eq!(block_ranges.last().unwrap().end, source.len(), "{report}");
        for pair in block_ranges.windows(2) {
            assert_eq!(pair[0].end, pair[1].start, "{report}");
        }
        let whole: String = block_ranges
            .iter()
            .map(|range| &source[range.clone()])
            .collect();
        assert_eq!(whole, source, "{report}");
    }
}
