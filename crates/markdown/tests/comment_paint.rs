use markdown::{CommentWashes, default_comment};
use theme::{Appearance, Theme};

#[test]
fn the_active_wash_is_stronger_than_the_open_one_and_resolved_the_faintest() {
    for appearance in [Appearance::Light, Appearance::Dark] {
        let washes = default_comment(&Theme::for_appearance(appearance));
        assert!(washes.active.a > washes.open.a);
        assert!(washes.open.a > washes.resolved.a);
    }
}

#[test]
fn washes_are_cut_from_their_colour() {
    let theme = Theme::for_appearance(Appearance::Dark);
    let washes = CommentWashes::of(theme.accent);
    assert_eq!(washes.color, theme.accent);
    assert_eq!(washes.active.h, theme.accent.h);
    assert_eq!(default_comment(&theme).color, theme.warning);
}
