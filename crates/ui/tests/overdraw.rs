use gpui::{Hsla, TextRun, font, hsla};
use ui::input::overdraw::{Overdraw, overdraws};

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
    assert!(overdraws(&[run(10, WHITE)], PAGE, &[]).is_empty());
}

#[test]
fn text_that_reads_over_its_layer_is_left_alone() {
    let dim = hsla(0.6, 0.4, 0.3, 1.0);
    assert!(overdraws(&[run(10, WHITE)], PAGE, &[(2..6, dim)]).is_empty());
}

#[test]
fn only_the_covered_piece_is_drawn_again() {
    let pale = hsla(0.6, 1.0, 0.85, 1.0);
    let pieces = overdraws(&[run(4, WHITE), run(6, WHITE)], PAGE, &[(2..7, pale)]);
    assert_eq!(pieces.len(), 1, "{pieces:?}");
    let Overdraw { range, plate, ink } = &pieces[0];
    assert_eq!(*range, 2..7);
    assert_eq!(*plate, theme::stack(PAGE, [pale]));
    assert_ne!(*ink, WHITE);
}

#[test]
fn a_piece_under_two_layers_takes_their_stack() {
    let pale = hsla(0.6, 1.0, 0.85, 1.0);
    let wash = hsla(0.15, 1.0, 0.5, 0.4);
    let pieces = overdraws(&[run(10, WHITE)], PAGE, &[(0..10, pale), (5..10, wash)]);
    assert_eq!(pieces.len(), 2, "{pieces:?}");
    assert_eq!(pieces[0].range, 0..5);
    assert_eq!(pieces[1].range, 5..10);
    assert_eq!(pieces[1].plate, theme::stack(PAGE, [pale, wash]));
}
