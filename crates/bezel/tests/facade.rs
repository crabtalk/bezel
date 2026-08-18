//! The facade guarantee: every layer reached through `bezel::*` speaks one gpui.

/// The guarantee the facade exists for: everything reached through
/// `bezel::*` speaks the *same* gpui. If a second copy ever entered the
/// graph these annotations would stop type-checking — which is the failure
/// worth catching at compile time, because at runtime it shows up as a
/// window that paints shapes but no text.
#[test]
fn every_layer_shares_one_gpui() {
    let theme = bezel::theme::Theme::dark();
    let _: bezel::gpui::Hsla = theme.bg;
    let _: bezel::gpui::SharedString = theme.font_sans.clone();

    let _: bezel::gpui::Hsla = bezel::theme::ink(0.05);
    let _: bezel::gpui::Hsla = bezel::motion::mix(theme.bg, theme.text, 0.5);

    // The components layer, reached through the facade, styles with the
    // same tokens.
    let _: bezel::gpui::Hsla = bezel::ui::popover::band();
}

/// Motion's catalog is reachable without naming `bezel-motion` directly.
#[test]
fn motion_catalog_is_reachable() {
    assert_eq!(bezel::motion::MENU_IN.duration_ms, 140);
    assert!(bezel::motion::MENU_OUT.total() < bezel::motion::MENU_IN.total());
}
