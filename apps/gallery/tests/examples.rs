//! The catalog declares a page's examples; the page paints them. A doc page's
//! ` ```rust example=<key> ` names one to put in its preview, so a key on one
//! side and not the other is a preview that silently shows the wrong thing.

use gallery::{Gallery, Section, TABS};
use gpui::{TestAppContext, VisualTestContext, px, size};

fn all_sections() -> impl Iterator<Item = &'static Section> {
    TABS.iter()
        .flat_map(|tab| tab.groups)
        .flat_map(|group| group.sections)
}

/// Two examples with the same key: `?e=` would embed the first and the second
/// would be unreachable.
#[test]
fn example_keys_are_unique_within_a_section() {
    for section in all_sections() {
        let mut seen = std::collections::HashSet::new();
        for example in section.examples {
            assert!(
                seen.insert(example.key),
                "{} declares {} twice",
                section.key,
                example.key
            );
        }
    }
}

/// A section is either one undivided body or two or more named examples. One
/// named example is a page that gained the ceremony without the split.
#[test]
fn a_split_section_has_more_than_one_example() {
    for section in all_sections() {
        assert_ne!(
            section.examples.len(),
            1,
            "{} declares a single example; either split it or declare none",
            section.key
        );
    }
}

/// Both directions: a declared key with no demo behind it is an empty preview,
/// and a painted key nobody declared is a demo no doc page can reach.
#[gpui::test]
fn declared_examples_are_the_ones_painted(cx: &mut TestAppContext) {
    cx.update(|cx| {
        ui::register_fonts(cx).ok();
        // `Theme::install` rather than `appearance::init`: the platform read
        // behind the latter wants a real NSAppearance, which a headless test
        // has no window server to hand it.
        theme::Theme::install(theme::Appearance::Dark, cx);
        gallery::init(cx);
    });
    let window = cx.add_window(|_, cx| Gallery::new(cx));
    let gallery = window.root(cx).expect("gallery window");
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(1000.0), px(860.0)));

    for section in all_sections().filter(|section| !section.examples.is_empty()) {
        let painted = gallery.update_in(&mut cx, |gallery, window, cx| {
            gallery.painted(section, window, cx)
        });
        let declared: Vec<_> = section.examples.iter().map(|example| example.key).collect();
        assert_eq!(
            painted, declared,
            "{} paints examples the catalog does not declare, or the other way round",
            section.key
        );
    }
}
