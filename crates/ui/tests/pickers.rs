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

// ---------------------------------------------------------------------------
// A result list long enough to need a bottom
// ---------------------------------------------------------------------------

/// Taller than a capped list and than every list below it, so the window is
/// never what cuts one short.
const TALL: gpui::Pixels = px(1200.0);
/// Fine enough to land inside a row of any height the test system shapes.
const STEP: f32 = 4.0;

/// A palette over `count` items, drawn in a window taller than its list.
fn long_palette(
    cx: &mut TestAppContext,
    count: usize,
) -> (Entity<CommandPalette>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        input::init(cx);
        palette::init(cx);
    });
    let items = (0..count)
        .map(|item| gpui::SharedString::from(format!("item {item}")))
        .collect();
    let window = cx.add_window(|_, cx| CommandPalette::new(items, cx));
    let palette = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(600.0), TALL));
    visual.update(|window, cx| palette.update(cx, |palette, cx| palette.focus(window, cx)));
    visual.run_until_parked();
    (palette, visual)
}

/// Every item the pointer can reach by walking down the palette — which is
/// every row actually painted, since a row past the list's bottom is clipped
/// and answers nothing.
fn reachable(palette: &Entity<CommandPalette>, cx: &mut VisualTestContext) -> Vec<usize> {
    let mut seen = Vec::new();
    for step in 0..(f32::from(TALL) / STEP) as usize {
        let at = gpui::point(px(100.0), px(step as f32 * STEP));
        cx.simulate_mouse_move(at, gpui::MouseButton::Left, gpui::Modifiers::default());
        if let Some(item) = cx.update(|_, cx| palette.read(cx).active_item())
            && !seen.contains(&item)
        {
            seen.push(item);
        }
    }
    seen
}

/// How many rows a palette over `count` items paints.
fn rows_shown(cx: &mut TestAppContext, count: usize) -> usize {
    let (palette, mut cx) = long_palette(cx, count);
    reachable(&palette, &mut cx).len()
}

#[gpui::test]
fn two_long_lists_show_the_same_number_of_rows(cx: &mut TestAppContext) {
    // The cap decides, not the item count and not the window: both of these
    // would otherwise paint every row they have, one of them past the bottom
    // edge where nothing can be read or reached.
    let thirty = rows_shown(cx, 30);
    let hundred = rows_shown(cx, 100);
    assert!(thirty > 0, "no row answered the pointer at all");
    assert_eq!(
        thirty, hundred,
        "a list of 100 painted {hundred} rows and one of 30 painted {thirty}"
    );
}

#[gpui::test]
fn a_list_shorter_than_the_cap_is_left_alone(cx: &mut TestAppContext) {
    // A ceiling, not a height: five results do not open a card sized for
    // twelve.
    assert_eq!(rows_shown(cx, 5), 5);
}

#[gpui::test]
fn the_arrows_keep_the_active_row_in_view(cx: &mut TestAppContext) {
    let (palette, mut cx) = long_palette(cx, 100);
    for _ in 0..40 {
        cx.simulate_keystrokes("down");
    }
    let active = cx
        .update(|_, cx| palette.read(cx).active_item())
        .expect("nothing active after walking down the list");

    // The sweep hovers rows as it goes, so read the active row first.
    let reachable = reachable(&palette, &mut cx);
    assert!(
        reachable.contains(&active),
        "the arrows walked to row {active}, which is painted nowhere: {reachable:?}"
    );
    assert!(
        !reachable.contains(&0),
        "the list never moved — row 0 is still on screen with row {active} selected"
    );
}
