//! Syntax colours on a field: the runs spans become, and what an edit does to
//! them.

use gpui::{TestAppContext, TextRun, font};
use theme::{Appearance, HighlightKind, SyntaxPalette, Theme};
use ui::input::{TextField, coloured, underlined};

fn base() -> TextRun {
    TextRun {
        len: 0,
        font: font("Helvetica"),
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

fn palette(cx: &mut TestAppContext) -> SyntaxPalette {
    cx.update(|cx| {
        Theme::install(Appearance::Dark, cx);
        Theme::of(cx).syntax.clone()
    })
}

/// Shaping walks the runs and the text together, so their lengths have to add
/// up to the text however odd the spans are.
fn covers(text: &str, runs: &[TextRun]) {
    assert_eq!(
        runs.iter().map(|run| run.len).sum::<usize>(),
        text.len(),
        "runs must cover the text exactly"
    );
}

#[gpui::test]
fn spans_colour_themselves_and_leave_the_gaps_alone(cx: &mut TestAppContext) {
    let palette = palette(cx);
    let text = "let x = 1;";
    let runs = coloured(
        text,
        &[
            (0..3, HighlightKind::Keyword),
            (8..9, HighlightKind::Number),
        ],
        &base(),
        &palette,
    );
    covers(text, &runs);
    assert_eq!(
        runs.iter().map(|run| run.len).collect::<Vec<_>>(),
        vec![3, 5, 1, 1],
        "keyword, the gap, the number, the tail"
    );
    assert_eq!(runs[0].color, palette.color(HighlightKind::Keyword));
    assert_eq!(runs[1].color, base().color, "the gap is the field's own");
    assert_eq!(runs[2].color, palette.color(HighlightKind::Number));
}

#[gpui::test]
fn text_with_no_spans_is_one_run(cx: &mut TestAppContext) {
    let palette = palette(cx);
    let runs = coloured("plain", &[], &base(), &palette);
    assert_eq!(runs.len(), 1);
    covers("plain", &runs);
}

/// A frame between an edit and the recolour paints ranges the text has
/// outgrown. Every one of these is dropped rather than shifting the runs after
/// it — the colour is wrong for a frame, which is the whole cost.
#[gpui::test]
fn spans_the_text_has_outgrown_are_dropped(cx: &mut TestAppContext) {
    let palette = palette(cx);
    let text = "let x = 1;";
    for spans in [
        vec![
            (0..3, HighlightKind::Keyword),
            (400..999, HighlightKind::Number),
        ],
        vec![
            (0..6, HighlightKind::Keyword),
            (2..4, HighlightKind::Number),
        ],
        vec![(5..5, HighlightKind::Number)],
        vec![(0..text.len() + 1, HighlightKind::String)],
    ] {
        let runs = coloured(text, &spans, &base(), &palette);
        covers(text, &runs);
    }
}

/// A run may not end inside a character: the font runs shaping builds from
/// these are byte lengths into the same string.
#[gpui::test]
fn spans_never_cut_a_character_in_half(cx: &mut TestAppContext) {
    let palette = palette(cx);
    let text = "let 中文 = 1;";
    let runs = coloured(text, &[(4..5, HighlightKind::String)], &base(), &palette);
    covers(text, &runs);
    assert_eq!(runs.len(), 1, "the span lands mid-character and is dropped");
}

#[gpui::test]
fn the_composition_range_underlines_whatever_colour_is_there(cx: &mut TestAppContext) {
    let palette = palette(cx);
    let text = "let x = 1;";
    let runs = coloured(text, &[(0..3, HighlightKind::Keyword)], &base(), &palette);
    let runs = underlined(runs, &(1..5));
    covers(text, &runs);
    let marked: Vec<_> = runs
        .iter()
        .scan(0, |at, run| {
            let start = *at;
            *at += run.len;
            Some((start..*at, run))
        })
        .filter(|(_, run)| run.underline.is_some())
        .map(|(range, run)| (range, run.color))
        .collect();
    assert_eq!(
        marked
            .iter()
            .map(|(range, _)| range.clone())
            .collect::<Vec<_>>(),
        vec![1..3, 3..5],
        "the marked range is underlined across the colour boundary"
    );
    assert_eq!(
        marked[0].1,
        palette.color(HighlightKind::Keyword),
        "and keeps the colour it had"
    );
}

#[gpui::test]
fn replacing_the_content_drops_the_colours(cx: &mut TestAppContext) {
    cx.update(|cx| Theme::install(Appearance::Dark, cx));
    let window = cx.add_window(|_, cx| TextField::new(cx));
    window
        .update(cx, |field, _, cx| {
            field.set_content("let x = 1;", cx);
            field.set_spans(vec![(0..3, HighlightKind::Keyword)], cx);
            assert_eq!(field.spans().len(), 1);
            // The colours were about text this field no longer holds.
            field.set_content("let y = 2;", cx);
            assert!(field.spans().is_empty());
        })
        .unwrap();
}
