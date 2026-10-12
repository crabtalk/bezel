//! A document kept as markdown and as blocks answers what reading the
//! markdown afresh does, however it was edited.

use std::rc::Rc;

use markdown::{Cursor, Doc, Marks, Part, Synced, parse_at_many, parse_ranges_with, parse_with};

mod common;

fn document(rng: &mut common::Rng) -> String {
    let parts = 1 + rng.next() % 6;
    (0..parts)
        .map(|_| match rng.next() % 2 {
            0 => common::BLOCKS[rng.next() % common::BLOCKS.len()],
            _ => common::LINES[rng.next() % common::LINES.len()],
        })
        .collect::<Vec<_>>()
        .join(["\n", "\n\n"][rng.next() % 2])
}

/// The ranges partition the text, one per block.
fn assert_partition(synced: &Synced, report: &str) {
    let ranges = synced.ranges();
    assert_eq!(ranges.len(), synced.doc().blocks.len(), "{report}");
    if let (Some(first), Some(last)) = (ranges.first(), ranges.last()) {
        assert_eq!(first.start, 0, "{report}");
        assert_eq!(last.end, synced.text().len(), "{report}");
    }
    for pair in ranges.windows(2) {
        assert_eq!(pair[0].end, pair[1].start, "{report}");
    }
}

/// The text reads back as the blocks, normalized. Where normalizing does not
/// settle in one round, it reads back as the blocks normalized twice, or
/// normalized itself it matches them.
fn assert_reads_back(synced: &Synced, marks: &Marks, report: &str) {
    let mut normalized = (**synced.doc()).clone();
    normalized.normalize_with(marks);
    let read = parse_with(synced.text(), marks);
    if read == normalized {
        return;
    }
    let mut again = normalized.clone();
    again.normalize_with(marks);
    if read == again {
        return;
    }
    let mut read = read;
    read.normalize_with(marks);
    assert_eq!(
        read,
        normalized,
        "{report}--- text ---\n{}\n--- whole ---\n{}\n",
        synced.text(),
        markdown::serialize_with(synced.doc(), marks),
    );
}

#[test]
fn block_edits_rewrite_only_what_reads_back_as_the_blocks() {
    let marks = Marks::default();
    let mut rng = common::Rng(0x5c);
    for case in 0..4_000 {
        let source = document(&mut rng);
        let mut synced = Synced::new(source.clone(), &marks);
        let mut doc: Doc = (**synced.doc()).clone();
        for step in 0..6 {
            common::edit_randomly(&mut doc, &mut rng);
            synced.set_doc(Rc::new(doc.clone()));
            let report = format!("case {case} step {step}\n--- source ---\n{source}\n");
            assert_partition(&synced, &report);
            assert_reads_back(&synced, &marks, &report);
        }
    }
}

#[test]
fn an_untouched_block_keeps_its_bytes() {
    let marks = Marks::default();
    let source = "*  first\n\nSetext\n=====\n\nlast";
    let mut synced = Synced::new(source, &marks);
    let mut doc: Doc = (**synced.doc()).clone();
    doc.edit_at(Cursor::new(2, Part::Body, 0), |text| text.insert(4, "!"));
    synced.set_doc(Rc::new(doc));
    assert!(synced.text().starts_with("*  first"), "{}", synced.text());
    assert!(synced.text().ends_with("last!"), "{}", synced.text());
}

#[test]
fn text_edits_read_back_as_a_fresh_parse() {
    text_edits_read_back(&Marks::default(), 0x7e);
}

#[test]
fn text_edits_with_custom_marks_read_back_as_a_fresh_parse() {
    text_edits_read_back(
        &Marks::new().with("highlight", "==").with("underline", "++"),
        0x3a,
    );
}

fn text_edits_read_back(marks: &Marks, seed: u64) {
    let mut rng = common::Rng(seed);
    for case in 0..4_000 {
        let source = document(&mut rng);
        let mut synced = Synced::new(source.clone(), marks);
        let mut text = source.clone();
        for step in 0..6 {
            let piece = match rng.next() % 4 {
                0 => ["==lit==", "a ++u++ b", "==", "\\=="][rng.next() % 4],
                _ => common::LINES[rng.next() % common::LINES.len()],
            };
            let boundaries: Vec<usize> = (0..=text.len())
                .filter(|&at| text.is_char_boundary(at))
                .collect();
            let a = boundaries[rng.next() % boundaries.len()];
            let b = boundaries[rng.next() % boundaries.len()];
            let (a, b) = (a.min(b), a.max(b));
            let insert = match rng.next() % 3 {
                0 => String::new(),
                1 => format!("\n{piece}"),
                _ => piece.to_owned(),
            };
            text.replace_range(a..b, &insert);
            synced.set_text(&text);

            let report = format!(
                "case {case} step {step}\n--- source ---\n{source}\n--- text ---\n{text}\n"
            );
            let fresh = parse_ranges_with(&text, marks);
            assert_eq!(**synced.doc(), fresh.doc, "{report}");
            assert_partition(&synced, &report);
            // Where on its line the parser starts a block spelled inside the
            // next one's bytes turns on what it read before; the line does not.
            let line = |at: usize| text[..at].matches('\n').count();
            for (kept, read) in synced.ranges().iter().zip(&fresh.block_ranges) {
                assert_eq!(line(kept.start), line(read.start), "{report}");
            }
        }
    }
}

#[test]
fn positions_map_as_the_whole_document_maps_them() {
    let marks = Marks::default();
    let mut rng = common::Rng(0x0ff);
    for case in 0..2_000 {
        let source = document(&mut rng);
        let synced = Synced::new(source.clone(), &marks);
        let offsets: Vec<usize> = (0..4).map(|_| rng.next() % (source.len() + 1)).collect();
        let report = format!("case {case}\n--- source ---\n{source}\n{offsets:?}\n");
        // Whether a sentinel changes what the source means can turn on the
        // other sentinels placed with it, so only a position both place is
        // compared.
        for (window, whole) in synced
            .cursors(&offsets)
            .into_iter()
            .zip(parse_at_many(&source, &offsets, &marks).1)
        {
            if window.is_some() && whole.is_some() {
                assert_eq!(window, whole, "{report}");
            }
        }
    }
}

#[test]
fn offsets_land_where_their_cursors_read_back() {
    let marks = Marks::default();
    let mut rng = common::Rng(0xa7);
    for case in 0..2_000 {
        let source = document(&mut rng);
        let mut synced = Synced::new(source.clone(), &marks);
        let doc = synced.doc().clone();
        let at: Vec<Cursor> = (0..3)
            .filter_map(|_| {
                let block = rng.next() % doc.blocks.len().max(1);
                let parts = doc.blocks.get(block)?.parts();
                let part = *parts.get(rng.next() % parts.len().max(1))?;
                let len = doc.blocks[block].text_at(part)?.text.len();
                let offset = rng.next() % (len + 1);
                Some(Cursor::new(block, part, offset))
            })
            .filter(|at| {
                doc.blocks[at.block]
                    .text_at(at.part)
                    .is_some_and(|text| text.text.is_char_boundary(at.offset))
            })
            .collect();
        let offsets = synced.offsets(&at);
        let report = format!(
            "case {case}\n--- source ---\n{source}\n--- text ---\n{}\n{at:?} -> {offsets:?}\n",
            synced.text()
        );
        assert_partition(&synced, &report);
        assert_reads_back(&synced, &marks, &report);
        let placed: Vec<usize> = offsets.iter().flatten().copied().collect();
        let (read, back) = parse_at_many(synced.text(), &placed, &marks);
        // Block numbers only line up while reading back keeps every block.
        if read.blocks.len() != doc.blocks.len() {
            continue;
        }
        let mut back = back.into_iter();
        for (cursor, offset) in at.iter().zip(&offsets) {
            // An offset the parser cannot place a sentinel at says nothing.
            if offset.is_some()
                && let Some(read) = back.next().flatten()
            {
                assert_eq!(read.block, cursor.block, "{report}");
            }
        }
    }
}
