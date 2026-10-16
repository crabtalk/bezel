use gpui::{Hsla, TextRun, font, hsla};
use ui::input::overdraw::{Layers, Overdraw, overdraws};

fn run(len: usize, color: Hsla) -> TextRun {
    TextRun {
        len,
        font: font("Mono"),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

const WHITE: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.95,
    a: 1.0,
};
const PAGE: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.1,
    a: 1.0,
};

#[test]
fn text_under_no_layer_is_left_alone() {
    assert!(overdraws(&[run(10, WHITE)], PAGE, &Layers::<()>::new()).is_empty());
}

#[test]
fn text_that_reads_over_its_layer_is_left_alone() {
    let dim = hsla(0.6, 0.4, 0.3, 1.0);
    assert!(
        overdraws(
            &[run(10, WHITE)],
            PAGE,
            &Layers::new().select(2..6, dim, ())
        )
        .is_empty()
    );
}

#[test]
fn only_the_covered_piece_is_drawn_again() {
    let pale = hsla(0.6, 1.0, 0.85, 1.0);
    let pieces = overdraws(
        &[run(4, WHITE), run(6, WHITE)],
        PAGE,
        &Layers::new().wash(2..7, pale, ()),
    );
    assert_eq!(pieces.len(), 1, "{pieces:?}");
    let Overdraw { range, plate, ink } = &pieces[0];
    assert_eq!(*range, 2..7);
    assert_eq!(*plate, theme::stack(PAGE, [pale]));
    assert_ne!(*ink, WHITE);
}

#[test]
fn a_piece_under_two_washes_takes_their_stack() {
    let pale = hsla(0.6, 1.0, 0.85, 1.0);
    let wash = hsla(0.15, 1.0, 0.5, 0.4);
    let pieces = overdraws(
        &[run(10, WHITE)],
        PAGE,
        &Layers::new().wash(0..10, pale, ()).wash(5..10, wash, ()),
    );
    assert_eq!(pieces.len(), 2, "{pieces:?}");
    assert_eq!(pieces[0].range, 0..5);
    assert_eq!(pieces[1].range, 5..10);
    assert_eq!(pieces[1].plate, theme::stack(PAGE, [pale, wash]));
}

#[test]
fn a_selection_over_a_wash_shows_both_under_the_text() {
    let selection = hsla(0.67, 0.7, 0.6, 1.0);
    let mark = hsla(0.14, 1.0, 0.5, 1.0);
    let pieces = overdraws(
        &[run(10, WHITE)],
        PAGE,
        &Layers::new()
            .wash(3..6, mark, ())
            .select(0..10, selection, ()),
    );
    let piece = pieces
        .iter()
        .find(|piece| piece.range == (3..6))
        .expect("{pieces:?}");
    let over = theme::selection_over(selection);
    assert_eq!(piece.plate, theme::stack(PAGE, [selection, mark, over]));
    assert_ne!(piece.plate, theme::stack(PAGE, [mark]));
    assert_ne!(piece.plate, theme::stack(PAGE, [selection]));
}

#[test]
fn text_coloured_for_its_wash_reads_under_a_selection() {
    // A highlight's dark text, which barely reads on the dark page.
    let dark = hsla(0.0, 0.0, 0.12, 1.0);
    let selection = hsla(0.67, 0.6, 0.3, 1.0);
    let mark = hsla(0.14, 1.0, 0.5, 1.0);
    let pieces = overdraws(
        &[run(10, dark)],
        PAGE,
        &Layers::new()
            .wash(0..10, mark, ())
            .select(0..10, selection, ()),
    );
    let over = theme::selection_over(selection);
    let plate = theme::stack(PAGE, [selection, mark, over]);
    let ink = pieces.first().map_or(dark, |piece| piece.ink);
    let reads = theme::contrast_ratio(theme::flatten(ink, plate), plate);
    assert!(reads >= theme::LEGIBLE, "{reads:.2}");
}

#[test]
fn layers_paint_the_selection_under_the_washes_and_again_over_them() {
    let selection = hsla(0.67, 0.7, 0.6, 1.0);
    let mark = hsla(0.14, 1.0, 0.5, 1.0);
    let layers = Layers::new()
        .select(0..4, selection, 'S')
        .wash(1..3, mark, 'W');
    let painted: Vec<_> = layers
        .painted()
        .map(|(range, color, shape)| (range.clone(), color, *shape))
        .collect();
    assert_eq!(
        painted,
        [
            (0..4, selection, 'S'),
            (1..3, mark, 'W'),
            (0..4, theme::selection_over(selection), 'S'),
        ]
    );
}
