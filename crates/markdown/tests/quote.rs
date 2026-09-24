use markdown::{Cursor, Part, Quote, Selection, parse};

fn body(block: usize, from: usize, to: usize) -> Selection {
    Selection::new(
        Cursor::new(block, Part::Body, from),
        Cursor::new(block, Part::Body, to),
    )
}

#[test]
fn a_quote_keeps_the_text_and_its_context() {
    let doc = parse("alpha one\n\nbravo two");
    let quote = Quote::of(&doc, body(1, 6, 9)).unwrap();
    assert_eq!(quote.exact, "two");
    assert_eq!(quote.prefix, "alpha one\nbravo ");
    assert_eq!(quote.suffix, "");
}

#[test]
fn a_collapsed_range_has_no_quote() {
    let doc = parse("alpha");
    assert_eq!(Quote::of(&doc, body(0, 2, 2)), None);
}

#[test]
fn the_hint_wins_while_it_still_covers_the_text() {
    let doc = parse("same same same");
    let quote = Quote::of(&doc, body(0, 5, 9)).unwrap();
    assert_eq!(quote.find(&doc, Some(body(0, 5, 9))), Some(body(0, 5, 9)));
}

#[test]
fn text_moved_by_an_outside_edit_is_found_again() {
    let before = parse("alpha one\n\nbravo two");
    let range = body(1, 6, 9);
    let quote = Quote::of(&before, range).unwrap();

    let after = parse("a new first line\n\nalpha one\n\nbravo, then two");
    assert_eq!(quote.find(&after, Some(range)), Some(body(2, 12, 15)));
}

#[test]
fn context_picks_between_repeats() {
    let before = parse("the cat sat\n\nthe cat ran");
    let quote = Quote::of(&before, body(1, 4, 7)).unwrap();

    // Both lines moved, so the hint covers neither; the suffix ` ran` decides.
    let after = parse("intro\n\nthe cat sat\n\nthe cat ran");
    assert_eq!(quote.find(&after, None), Some(body(2, 4, 7)));
}

#[test]
fn a_range_across_blocks_is_quoted_and_found() {
    let before = parse("first\n\nsecond");
    let range = Selection::new(Cursor::new(0, Part::Body, 2), Cursor::new(1, Part::Body, 3));
    let quote = Quote::of(&before, range).unwrap();
    assert_eq!(quote.exact, "rst\nsec");

    let after = parse("zero\n\nfirst\n\nsecond");
    assert_eq!(
        quote.find(&after, None),
        Some(Selection::new(
            Cursor::new(1, Part::Body, 2),
            Cursor::new(2, Part::Body, 3)
        ))
    );
}

#[test]
fn text_that_is_gone_is_not_found() {
    let before = parse("alpha one");
    let quote = Quote::of(&before, body(0, 6, 9)).unwrap();
    assert_eq!(quote.find(&parse("alpha"), Some(body(0, 6, 9))), None);
}

#[test]
fn a_stale_hint_inside_a_character_does_not_panic() {
    let quote = Quote::of(&parse("ab"), body(0, 1, 2)).unwrap();
    assert_eq!(
        quote.find(&parse("éb"), Some(body(0, 1, 2))),
        Some(body(0, 2, 3))
    );
}
