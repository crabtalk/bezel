use gpui::{Entity, Focusable, TestAppContext, VisualTestContext, px, size};
use ui::{
    combobox::{self, Combobox},
    input,
    palette::{self, CommandPalette},
};

fn combo(cx: &mut TestAppContext) -> (Entity<Combobox>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        input::init(cx);
        combobox::init(cx);
    });
    let window =
        cx.add_window(|_, cx| Combobox::new(vec!["apple".into(), "apricot".into()], "Fruit", cx));
    let combo = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(400.0), px(300.0)));
    visual.update(|window, cx| combo.read(cx).focus_handle(cx).focus(window, cx));
    visual.run_until_parked();
    (combo, visual)
}

#[gpui::test]
fn closed_combobox_opens_before_it_commits_and_restores_focus(cx: &mut TestAppContext) {
    let (combo, mut cx) = combo(cx);
    cx.simulate_keystrokes("enter");
    assert_eq!(cx.update(|_, cx| combo.read(cx).selection()), None);
    cx.simulate_keystrokes("down enter");
    assert_eq!(cx.update(|_, cx| combo.read(cx).selection()), Some(1));
    assert!(cx.update(|window, cx| combo.read(cx).focus_handle(cx).is_focused(window)));
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    cx.simulate_keystrokes("escape");
    assert_eq!(cx.update(|_, cx| combo.read(cx).selection()), Some(1));
    assert!(cx.update(|window, cx| combo.read(cx).focus_handle(cx).is_focused(window)));
}

#[gpui::test]
fn moving_the_query_caret_preserves_combobox_highlight(cx: &mut TestAppContext) {
    let (combo, mut cx) = combo(cx);
    cx.simulate_keystrokes("down");
    cx.simulate_input("ap");
    cx.simulate_keystrokes("down left enter");
    assert_eq!(cx.update(|_, cx| combo.read(cx).selection()), Some(1));
}

#[gpui::test]
fn moving_the_query_caret_preserves_palette_highlight(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        input::init(cx);
        palette::init(cx);
    });
    let window =
        cx.add_window(|_, cx| CommandPalette::new(vec!["apple".into(), "apricot".into()], cx));
    let palette = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(500.0), px(400.0)));
    cx.update(|window, cx| palette.update(cx, |palette, cx| palette.focus(window, cx)));
    cx.run_until_parked();
    cx.simulate_input("ap");
    cx.simulate_keystrokes("down left");
    assert_eq!(cx.update(|_, cx| palette.read(cx).active_item()), Some(1));
}
