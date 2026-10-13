//! What the generative tests share: a seeded generator, and the pieces they
//! build documents and edits from.
#![allow(dead_code)]

use markdown::{Cursor, Doc, Mark, Part, Selection, Text};

pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as usize
    }
}

/// Line fragments to shuffle into documents no one would think to write.
/// The point of the generative test is exactly the combinations a
/// hand-written corpus misses, so the awkward ones earn their place here.
pub const LINES: &[&str] = &[
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
    ">     - y",
    "> 1. one",
    "> ```",
    "> # h",
    "    > q",
    ">",
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

/// Whole blocks, to join with blank lines.
pub const BLOCKS: &[&str] = &[
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

const WORDS: &[&str] = &["x", " ", "\n", "a b", "**", "#", "- ", "`", "|", "~~"];
const MARKS: &[Mark] = &[Mark::Bold, Mark::Italic, Mark::Strike, Mark::Code];

/// One edit to `doc`, of the kinds an editor makes: typing, deleting, a mark,
/// a split, a merge, a cross-block replace, an indent or an outdent.
pub fn edit_randomly(doc: &mut Doc, rng: &mut Rng) {
    if doc.blocks.is_empty() {
        return;
    }
    let ix = rng.next() % doc.blocks.len();
    let len = doc.blocks[ix]
        .text_at(Part::Body)
        .map_or(0, |text| text.text.len());
    let a = if len == 0 { 0 } else { rng.next() % (len + 1) };
    let b = if len == 0 { 0 } else { rng.next() % (len + 1) };

    let at = Cursor::new(ix, Part::Body, 0);
    match rng.next() % 8 {
        0 => {
            let word = WORDS[rng.next() % WORDS.len()];
            doc.edit_at(at, |text| text.insert(a, word));
        }
        1 => {
            doc.edit_at(at, |text| text.remove(a.min(b)..a.max(b)));
        }
        2 => {
            let mark = MARKS[rng.next() % MARKS.len()].clone();
            doc.edit_at(at, |text| text.toggle(a.min(b)..a.max(b), mark));
        }
        3 => {
            doc.split(ix, a);
        }
        4 => {
            doc.merge_back(at);
        }
        // A selection over any two positions in the document —
        // where every cross-block delete, cut and paste lands.
        5 => {
            let jx = rng.next() % doc.blocks.len();
            let jlen = doc.blocks[jx]
                .text_at(Part::Body)
                .map_or(0, |text| text.text.len());
            let c = if jlen == 0 {
                0
            } else {
                rng.next() % (jlen + 1)
            };
            let word = WORDS[rng.next() % WORDS.len()];
            doc.replace(
                Selection::new(
                    Cursor::new(ix, Part::Body, a),
                    Cursor::new(jx, Part::Body, c),
                ),
                Text::plain(word),
            );
        }
        6 => {
            doc.indent(ix);
        }
        _ => {
            doc.outdent(ix);
        }
    }
}
