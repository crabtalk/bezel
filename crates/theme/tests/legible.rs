use gpui::{Hsla, hsla};
use theme::{LEGIBLE, Theme, contrast_ratio, flatten, ink_over, kept, legible, over, stack};

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
            let ink = ink_over(theme.text, theme.bg, theme.bg, plate);
            assert!(reads(ink, plate) >= LEGIBLE, "{:?}", theme.appearance);
        }
    }
}

#[test]
fn two_translucent_layers_composite_to_one_that_paints_the_same() {
    let page = hsla(0.6, 0.2, 0.3, 1.0);
    let under = hsla(0.0, 0.0, 0.9, 0.6);
    let top = hsla(0.0, 0.0, 0.1, 0.5);
    let one = over(top, under);
    assert!((one.a - 0.8).abs() < 1e-4);
    let (a, b) = (stack(page, [under, top]), stack(page, [one]));
    assert!((a.l - b.l).abs() < 1e-4 && (a.s - b.s).abs() < 1e-4);
}

#[test]
fn text_that_does_not_read_takes_the_colour_it_sits_on() {
    // crabtalk/cydonia#108: beige on a cyan selection or caret, over the page.
    let page = hsla(0.0, 0.0, 40.0 / 255.0, 1.0);
    let beige = hsla(0.11, 0.35, 0.72, 1.0);
    let cyan = hsla(0.49, 0.75, 0.62, 1.0);
    assert_eq!(ink_over(beige, page, page, cyan), page);
}
