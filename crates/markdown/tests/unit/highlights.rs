use crate::render::code::Highlights;
use theme::HighlightKind;

#[test]
fn indexed_spans_match_a_full_scan_at_line_boundaries() {
    let highlights = Highlights::from(vec![
        (0..2, HighlightKind::Keyword),
        (2..9, HighlightKind::String),
        (10..12, HighlightKind::Comment),
    ]);
    for start in 0..=12 {
        for end in start..=12 {
            let expected: Vec<_> = highlights
                .spans
                .iter()
                .filter(|(range, _)| range.end > start && range.start < end)
                .collect();
            assert_eq!(highlights.within(start, end).collect::<Vec<_>>(), expected);
        }
    }
}

#[test]
fn custom_spans_keep_their_order_when_ranges_are_nested() {
    let highlights = Highlights::from(vec![
        (0..12, HighlightKind::Keyword),
        (1..3, HighlightKind::String),
        (5..9, HighlightKind::Comment),
    ]);
    let spans: Vec<_> = highlights.within(5, 9).collect();
    assert_eq!(spans, [&highlights.spans[0], &highlights.spans[2]]);
}
