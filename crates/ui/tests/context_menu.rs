//! A view's keyed menus: toggled from their triggers, walked by key.

use gpui::{
    Context, Entity, FocusHandle, Modifiers, Render, TestAppContext, VisualTestContext, Window,
    div, prelude::*, px,
};
use std::time::Duration;
use theme::{Appearance, Theme};
use ui::{
    context_menu::{ContextMenu, Place},
    menu::Item,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    One,
    Two,
}

struct Page {
    menu: ContextMenu<Self, Key>,
    chosen: Vec<Vec<usize>>,
    focus: FocusHandle,
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let one = ContextMenu::trigger(
            div()
                .id("one")
                .debug_selector(|| "one".into())
                .relative()
                .w(px(80.0))
                .h(px(24.0))
                .on_click(cx.listener(|page, _, window, cx| open(page, Key::One, window, cx))),
            |page| &mut page.menu,
            Key::One,
            cx,
        )
        .children(self.menu.render_for(
            &Key::One,
            "one-menu",
            |page| &mut page.menu,
            window,
            cx,
        ));
        let two = ContextMenu::trigger(
            div()
                .id("two")
                .debug_selector(|| "two".into())
                .relative()
                .w(px(80.0))
                .h(px(24.0))
                .on_click(cx.listener(|page, _, window, cx| open(page, Key::Two, window, cx))),
            |page| &mut page.menu,
            Key::Two,
            cx,
        )
        .children(self.menu.render_for(
            &Key::Two,
            "two-menu",
            |page| &mut page.menu,
            window,
            cx,
        ));
        div()
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .gap(px(200.0))
            .child(one)
            .child(two)
    }
}

/// What a trigger's click does: toggle its menu of a row and a submenu.
fn open(page: &mut Page, key: Key, window: &mut Window, cx: &mut Context<Page>) {
    ContextMenu::toggle(
        page,
        |page| &mut page.menu,
        key,
        Place::Below,
        |_, _, _| {
            let items = vec![
                Item::action("First"),
                Item::submenu("More", vec![Item::action("Inner")]),
            ];
            let choose =
                |page: &mut Page, path: &[usize], _: &mut Window, _: &mut Context<Page>| {
                    page.chosen.push(path.to_vec())
                };
            (items, choose)
        },
        window,
        cx,
    );
}

fn page(cx: &mut TestAppContext) -> (Entity<Page>, VisualTestContext) {
    cx.update(|cx| {
        Theme::install(Appearance::Dark, cx);
        ui::menu::init(cx);
    });
    let window = cx.add_window(|_, cx| Page {
        menu: ContextMenu::default(),
        chosen: Vec::new(),
        focus: cx.focus_handle(),
    });
    let page = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.update(|window, cx| {
        let focus = page.read(cx).focus.clone();
        window.focus(&focus, cx)
    });
    cx.run_until_parked();
    (page, cx)
}

/// Click the element `selector` names, then wait out any menu's exit.
fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx.debug_bounds(selector).expect(selector).center();
    cx.simulate_click(at, Modifiers::default());
    settle(cx);
}

fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
}

fn open_key(page: &Entity<Page>, cx: &mut VisualTestContext) -> Option<Key> {
    page.read_with(cx, |page, _| page.menu.key().copied())
}

#[gpui::test]
fn a_trigger_toggles_its_own_menu(cx: &mut TestAppContext) {
    let (page, mut cx) = page(cx);
    click(&mut cx, "one");
    assert_eq!(open_key(&page, &mut cx), Some(Key::One));
    click(&mut cx, "one");
    assert_eq!(open_key(&page, &mut cx), None, "its own trigger shuts it");
    click(&mut cx, "one");
    assert_eq!(
        open_key(&page, &mut cx),
        Some(Key::One),
        "and opens it again"
    );
    // A press that dismisses an open card is spent on the dismissal.
    click(&mut cx, "two");
    assert_eq!(open_key(&page, &mut cx), None);
    click(&mut cx, "two");
    assert_eq!(open_key(&page, &mut cx), Some(Key::Two));
}

#[gpui::test]
fn the_keyboard_walks_into_a_submenu_and_backs_out_a_level_at_a_time(cx: &mut TestAppContext) {
    let (page, mut cx) = page(cx);
    click(&mut cx, "one");
    cx.simulate_keystrokes("down down right");
    settle(&mut cx);
    assert!(
        cx.debug_bounds("one-menu/Inner").is_some(),
        "the submenu is down"
    );
    cx.simulate_keystrokes("escape");
    settle(&mut cx);
    assert_eq!(
        open_key(&page, &mut cx),
        Some(Key::One),
        "escape left the menu up"
    );
    assert!(
        cx.debug_bounds("one-menu/Inner").is_none(),
        "but took the submenu"
    );
    cx.simulate_keystrokes("enter enter");
    settle(&mut cx);
    assert_eq!(open_key(&page, &mut cx), None);
    page.read_with(&cx, |page, _| assert_eq!(page.chosen, [vec![1, 0]]));
    cx.update(|window, cx| {
        assert!(
            page.read(cx).focus.is_focused(window),
            "the keys are handed back"
        )
    });
}
