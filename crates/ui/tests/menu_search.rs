//! A searchable submenu: the query it opens focused on, and the rows its keys
//! walk and choose among.

use gpui::{Context, TestAppContext, VisualTestContext, Window, div, prelude::*, px, size};
use ui::{
    input,
    menu::{self, Cursor, Hit, Item},
    popover,
};

fn items() -> Vec<Item> {
    vec![
        Item::submenu(
            "Link",
            vec![
                Item::action("alpha"),
                Item::action("beta").with_description("codex"),
                Item::action("gamma"),
            ],
        )
        .searchable("Search"),
    ]
}

struct Host {
    cursor: Cursor,
    chosen: Option<Vec<usize>>,
    dismissed: bool,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme::Theme::of(cx).clone();
        let menu = items();
        let card = menu::card(
            &theme,
            "m",
            &menu,
            &self.cursor,
            window,
            cx,
            |this: &mut Self, hit, _, cx| {
                match hit {
                    Hit::Point(path) => {
                        this.cursor.point_at(&items(), &path);
                    }
                    Hit::Choose(path) => this.chosen = Some(path),
                    Hit::Dismiss => this.dismissed = true,
                }
                cx.notify();
            },
        );
        div().size_full().child(popover::anchored_menu_below(
            "m",
            card.into_any_element(),
            None,
        ))
    }
}

fn open(cx: &mut TestAppContext) -> (gpui::Entity<Host>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        input::init(cx);
        menu::init(cx);
    });
    let window = cx.add_window(|_, _| {
        let mut cursor = Cursor::default();
        cursor.point_at(&items(), &[0]);
        Host {
            cursor,
            chosen: None,
            dismissed: false,
        }
    });
    let host = window.root(cx).unwrap();
    let visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(800.0), px(600.0)));
    visual.run_until_parked();
    (host, visual)
}

#[gpui::test]
fn the_query_narrows_the_rows_and_enter_takes_the_first(cx: &mut TestAppContext) {
    let (host, mut cx) = open(cx);
    cx.simulate_input("gam");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    assert_eq!(
        cx.update(|_, cx| host.read(cx).chosen.clone()),
        Some(vec![0, 2])
    );
}

#[gpui::test]
fn a_query_matches_the_description_too(cx: &mut TestAppContext) {
    let (host, mut cx) = open(cx);
    cx.simulate_input("codex");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    assert_eq!(
        cx.update(|_, cx| host.read(cx).chosen.clone()),
        Some(vec![0, 1])
    );
}

#[gpui::test]
fn the_arrows_walk_the_rows_on_show(cx: &mut TestAppContext) {
    let (host, mut cx) = open(cx);
    cx.simulate_keystrokes("down down enter");
    assert_eq!(
        cx.update(|_, cx| host.read(cx).chosen.clone()),
        Some(vec![0, 1])
    );
}

#[gpui::test]
fn escape_dismisses(cx: &mut TestAppContext) {
    let (host, mut cx) = open(cx);
    cx.simulate_keystrokes("escape");
    assert!(cx.update(|_, cx| host.read(cx).dismissed));
}
