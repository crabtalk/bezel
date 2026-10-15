use gpui::{Hsla, hsla};
use theme::{LEGIBLE, Theme, contrast_ratio, flatten, kept, legible, stack};

fn reads(text: Hsla, plate: Hsla) -> f32 {
    contrast_ratio(flatten(text, plate), plate)
}

#[test]
fn text_that_reads_keeps_its_colour() {
    for theme in [Theme::dark(), Theme::light()] {
        assert_eq!(legible(theme.text, theme.selection, LEGIBLE), theme.text);
    }
}

#[test]
fn text_that_does_not_read_is_turned_until_it_does() {
    let white = hsla(0.0, 0.0, 0.95, 1.0);
    let pale = hsla(0.6, 1.0, 0.85, 1.0);
    let ink = legible(white, pale, LEGIBLE);
    assert_ne!(ink, white);
    assert!(reads(ink, pale) >= LEGIBLE, "{:.2}", reads(ink, pale));
}

#[test]
fn a_mid_plate_falls_back_to_black_or_white() {
    let grey = hsla(0.0, 0.0, 0.5, 1.0);
    let ink = legible(grey, grey, LEGIBLE);
    assert!(ink == hsla(0.0, 0.0, 0.0, 1.0) || ink == hsla(0.0, 0.0, 1.0, 1.0));
}

#[test]
fn faint_text_is_held_to_its_own_contrast() {
    let theme = Theme::dark();
    let faint = theme.text.opacity(0.3);
    assert!(kept(faint, theme.bg) < LEGIBLE);
    assert!((kept(theme.text, theme.bg) - LEGIBLE).abs() < f32::EPSILON);
}

#[test]
fn layers_stack_in_order() {
    let base = hsla(0.0, 0.0, 0.0, 1.0);
    let white = hsla(0.0, 0.0, 1.0, 1.0);
    assert_eq!(stack(base, []), base);
    assert_eq!(stack(base, [white]), white);
    assert_eq!(stack(base, [white, base.opacity(0.0)]), white);
}

#[test]
fn every_selection_and_highlight_stack_keeps_body_text_legible() {
    for theme in [Theme::dark(), Theme::light()] {
        for wash in [theme.selection, theme.accent.opacity(0.45), theme.warning] {
            let plate = stack(theme.bg, [theme.selection, wash]);
            let ink = legible(theme.text, plate, kept(theme.text, theme.bg));
            assert!(reads(ink, plate) >= LEGIBLE, "{:?}", theme.appearance);
        }
    }
}
