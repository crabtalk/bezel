use markdown::{HighlightColor, default_highlight};
use theme::{Theme, contrast_ratio, flatten};

#[test]
fn text_reads_over_every_default_highlight() {
    for theme in [Theme::dark(), Theme::light()] {
        for color in HighlightColor::ALL {
            let wash = flatten(default_highlight(color, &theme), theme.bg);
            let ratio = contrast_ratio(theme.text, wash);
            assert!(
                ratio >= 7.0,
                "{:?} {color:?}: text at {ratio:.2}:1",
                theme.appearance
            );
        }
    }
}

#[test]
fn a_highlight_colour_round_trips_its_name() {
    for color in HighlightColor::ALL {
        assert_eq!(HighlightColor::from_name(color.name()), Some(color));
    }
    assert_eq!(HighlightColor::from_name("teal"), None);
}
