use bezel_ui::input::*;

/// A regional-indicator pair is one grapheme of 8 bytes: stepping by
/// `char` would land inside it and split the flag in half.
#[test]
fn boundaries_step_over_a_flag_emoji() {
    let text = "a🇯🇵b";
    assert_eq!(next_boundary(text, 0), 1, "past 'a'");
    assert_eq!(next_boundary(text, 1), 9, "over the whole flag");
    assert_eq!(previous_boundary(text, 9), 1, "back to the flag's start");
    assert_eq!(previous_boundary(text, 1), 0);
}

#[test]
fn boundaries_step_over_a_combining_mark() {
    // "e" + U+0301 combining acute = one grapheme, three bytes.
    let text = "e\u{301}x";
    assert_eq!(next_boundary(text, 0), 3, "over e+accent together");
    assert_eq!(previous_boundary(text, 3), 0);
}

#[test]
fn boundaries_clamp_at_both_ends() {
    let text = "hi";
    assert_eq!(previous_boundary(text, 0), 0, "no underflow");
    assert_eq!(next_boundary(text, text.len()), text.len(), "no overflow");
    assert_eq!(next_boundary("", 0), 0, "empty is inert");
    assert_eq!(previous_boundary("", 0), 0);
}

/// The IME addresses text in UTF-16, so every mapping has to survive
/// astral-plane characters (2 UTF-16 units, 4 bytes) and CJK (1 unit,
/// 3 bytes).
#[test]
fn utf16_offsets_round_trip() {
    for text in ["ascii", "日本語", "a😀b", "🇯🇵x", "e\u{301}"] {
        let mut byte = 0;
        for ch in text.chars() {
            let utf16 = offset_to_utf16(text, byte);
            assert_eq!(
                offset_from_utf16(text, utf16),
                byte,
                "{text:?} byte {byte} → utf16 {utf16} → back"
            );
            byte += ch.len_utf8();
        }
        assert_eq!(offset_to_utf16(text, byte), text.encode_utf16().count());
    }
}

/// A run of typing is one undo step, so `cmd-z` gives back the word rather
/// than the letter.
#[test]
fn typing_a_run_stays_one_undo_group() {
    let mut last = None;
    for offset in 1..6 {
        // Each insert lands exactly where the previous left the caret.
        assert!(
            joins_group(last, EditKind::Insert, offset - 1) || last.is_none(),
            "insert at {offset} should continue the run"
        );
        last = Some((EditKind::Insert, offset));
    }
}

/// The first edit has nothing to join.
#[test]
fn the_first_edit_opens_a_group() {
    assert!(!joins_group(None, EditKind::Insert, 0));
    assert!(!joins_group(None, EditKind::Delete, 7));
}

/// Moving the caret and typing somewhere else is a separate thought, and a
/// separate undo step.
#[test]
fn an_edit_elsewhere_starts_a_new_group() {
    let last = Some((EditKind::Insert, 5));
    assert!(
        joins_group(last, EditKind::Insert, 5),
        "same spot continues"
    );
    assert!(!joins_group(last, EditKind::Insert, 9), "moved away");
    assert!(!joins_group(last, EditKind::Insert, 4), "even by one");
}

/// Switching from typing to deleting breaks the group even without moving —
/// otherwise `cmd-z` after a backspace would give back the typing too.
#[test]
fn changing_edit_kind_starts_a_new_group() {
    assert!(!joins_group(
        Some((EditKind::Insert, 5)),
        EditKind::Delete,
        5
    ));
    assert!(!joins_group(
        Some((EditKind::Delete, 5)),
        EditKind::Insert,
        5
    ));
}

/// A `\r` that survived would shape as a glyph — `shape_text` splits on
/// `\n` alone — and put every offset after it out by one.
#[test]
fn normalize_folds_crlf_whatever_the_shape() {
    for shape in [Shape::Line, Shape::Rows(3), Shape::Grow { min: 1, max: 4 }] {
        let out = normalize("a\r\nb\rc", shape);
        assert!(!out.contains('\r'), "{shape:?} left a carriage return");
        assert_eq!(out.len(), 5, "{shape:?} changed the character count");
    }
}

/// The invariant a single-line field rests on: no newline, ever, however it
/// got there — and the pasted text is kept, not truncated at the break.
#[test]
fn normalize_keeps_a_single_line_single() {
    assert_eq!(normalize("a\nb", Shape::Line), "a b");
    assert_eq!(normalize("a\r\nb", Shape::Line), "a b");
    assert_eq!(normalize("one\ntwo\nthree", Shape::Line), "one two three");
}

#[test]
fn normalize_keeps_breaks_when_the_shape_has_room() {
    assert_eq!(normalize("a\nb", Shape::Rows(2)), "a\nb");
    assert_eq!(normalize("a\r\nb", Shape::Grow { min: 1, max: 3 }), "a\nb");
}

/// On a field that cannot hold a newline these collapse to the ends of the
/// content — which is what they did before logical lines existed, so a
/// single-line field is untouched by the change.
#[test]
fn line_bounds_on_one_line_are_the_whole_content() {
    let text = "the quick brown";
    for offset in [0, 4, text.len()] {
        assert_eq!(line_start(text, offset), 0);
        assert_eq!(line_end(text, offset), text.len());
    }
    assert_eq!(line_start("", 0), 0);
    assert_eq!(line_end("", 0), 0);
}

/// `ctrl-a`/`ctrl-e` stop at the newline, not at the ends of the buffer.
#[test]
fn line_bounds_are_bounded_by_newlines() {
    //           0123 4567 89
    let text = "one\ntwo\nup";
    assert_eq!(line_start(text, 0), 0, "first line starts at 0");
    assert_eq!(line_end(text, 0), 3, "and ends before the newline");
    assert_eq!(line_start(text, 5), 4, "past the newline, not on it");
    assert_eq!(line_end(text, 5), 7);
    assert_eq!(line_end(text, 8), text.len(), "last line runs to the end");
}

/// The cursor sitting on a newline belongs to the line that ends there —
/// `ctrl-e` must not jump it forward into the next one.
#[test]
fn line_bounds_at_a_newline_stay_put() {
    let text = "one\ntwo";
    assert_eq!(line_end(text, 3), 3, "already at the end, so no move");
    assert_eq!(line_start(text, 3), 0);
    assert_eq!(line_start(text, 4), 4, "start of the next line is itself");
}

/// Empty lines are lines: a run of newlines must not collapse.
#[test]
fn line_bounds_handle_empty_lines() {
    let text = "a\n\nb";
    assert_eq!(line_start(text, 2), 2, "the empty line between them");
    assert_eq!(line_end(text, 2), 2);
}

/// option-left lands on the start of the word you were in or just past,
/// option-right on the end of the next one.
#[test]
fn word_motion_walks_word_starts_and_ends() {
    let text = "the quick brown";
    assert_eq!(next_word_boundary(text, 0), 3, "end of 'the'");
    assert_eq!(
        next_word_boundary(text, 3),
        9,
        "skips the space, ends 'quick'"
    );
    assert_eq!(next_word_boundary(text, 6), 9, "from mid-word to its end");
    assert_eq!(previous_word_boundary(text, 15), 10, "start of 'brown'");
    assert_eq!(previous_word_boundary(text, 10), 4, "start of 'quick'");
    assert_eq!(
        previous_word_boundary(text, 6),
        4,
        "from mid-word to its start"
    );
}

/// Which punctuation splits a word is UAX#29's call, not ours, and the
/// answer is useful: identifiers stay whole, paths come apart.
#[test]
fn word_motion_keeps_identifiers_whole_but_splits_paths() {
    // A dot or underscore between letters does NOT break a word, so an
    // identifier is one motion.
    for identifier in ["foo.bar", "foo_bar"] {
        assert_eq!(
            next_word_boundary(identifier, 0),
            identifier.len(),
            "{identifier:?} is one word"
        );
        assert_eq!(previous_word_boundary(identifier, identifier.len()), 0);
    }

    // Hyphens and slashes do break.
    let text = "path/to/file";
    assert_eq!(next_word_boundary(text, 0), 4, "stops at the slash");
    assert_eq!(next_word_boundary(text, 4), 7, "then 'to'");
    assert_eq!(
        previous_word_boundary(text, text.len()),
        8,
        "back to 'file'"
    );
    assert_eq!(next_word_boundary("a-b", 0), 1, "hyphen breaks");
}

#[test]
fn word_motion_clamps_and_survives_runs_of_separators() {
    let text = "  a   b  ";
    assert_eq!(previous_word_boundary(text, 0), 0, "no underflow");
    assert_eq!(
        next_word_boundary(text, text.len()),
        text.len(),
        "no overflow"
    );
    assert_eq!(next_word_boundary(text, 0), 3, "over leading spaces to 'a'");
    assert_eq!(previous_word_boundary(text, text.len()), 6, "back to 'b'");
    // Nothing but separators: motion collapses to the ends, never panics.
    assert_eq!(next_word_boundary("   ", 0), 3);
    assert_eq!(previous_word_boundary("   ", 3), 0);
    assert_eq!(next_word_boundary("", 0), 0);
}

/// Word motion must not split a grapheme or land mid-character.
#[test]
fn word_motion_lands_on_char_boundaries() {
    for text in ["日本語 の テスト", "a😀b c", "🇯🇵 x"] {
        for offset in 0..=text.len() {
            if !text.is_char_boundary(offset) {
                continue;
            }
            let prev = previous_word_boundary(text, offset);
            let next = next_word_boundary(text, offset);
            assert!(text.is_char_boundary(prev), "{text:?} prev {prev}");
            assert!(text.is_char_boundary(next), "{text:?} next {next}");
            assert!(prev <= offset, "prev never moves forward");
            assert!(next >= offset, "next never moves back");
        }
    }
}

#[test]
fn utf16_offsets_count_surrogate_pairs_as_two() {
    // "😀" is one char, 4 bytes, but TWO UTF-16 code units.
    assert_eq!(offset_to_utf16("😀", 4), 2);
    assert_eq!(offset_from_utf16("😀", 2), 4);
    // CJK: 3 bytes, one unit.
    assert_eq!(offset_to_utf16("日", 3), 1);
    assert_eq!(offset_from_utf16("日", 1), 3);
}
