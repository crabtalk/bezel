use gpui::TestAppContext;
use theme::{Appearance, Theme};
use ui::{
    AppExt as _,
    color::{Swatch, default_swatches},
};

#[gpui::test]
fn the_set_is_the_default_until_the_app_replaces_it(cx: &mut TestAppContext) {
    cx.update(|cx| {
        assert_eq!(cx.color_swatches(), default_swatches());
        let set = [Swatch::fixed("Ink", gpui::rgb(0x111111))];
        cx.set_color_swatches(set.clone());
        assert_eq!(&*cx.color_swatches(), &set[..]);
    });
}

#[gpui::test]
fn a_swatch_resolves_by_appearance(cx: &mut TestAppContext) {
    let swatch = Swatch::new("Blue", gpui::rgb(0x007AFF), gpui::rgb(0x0A84FF));
    cx.update(|cx| {
        Theme::install(Appearance::Light, cx);
        assert_eq!(swatch.resolve(Theme::of(cx)), swatch.light);
        Theme::install(Appearance::Dark, cx);
        assert_eq!(swatch.resolve(Theme::of(cx)), swatch.dark);
    });
}

#[test]
fn every_system_colour_reads_back_by_its_name() {
    for color in ui::color::SystemColor::ALL {
        assert_eq!(ui::color::SystemColor::from_name(color.name()), Some(color));
    }
    assert_eq!(ui::color::SystemColor::from_name("magenta"), None);
}
