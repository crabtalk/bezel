//! The mention menus over an app's mention sources.

use editor::{AppExt as _, Chrome, Editor, Mention};
use gpui::{App, Entity, Focusable, TestAppContext, VisualTestContext, px, size};

fn source(query: &str, _: &App) -> Vec<Mention> {
    [("Roadmap", "app://roadmap"), ("Release", "app://release")]
        .into_iter()
        .filter(|(label, _)| label.to_lowercase().starts_with(&query.to_lowercase()))
        .map(|(label, url)| Mention {
            icon: None,
            label: label.into(),
            description: None,
            url: url.to_owned(),
        })
        .collect()
}

fn entries(query: &str, _: &App) -> Vec<Mention> {
    vec![Mention {
        icon: None,
        label: format!("Entry {query}").into(),
        description: None,
        url: format!("app://entry/{query}"),
    }]
}

fn open(mention: bool, cx: &mut TestAppContext) -> (Entity<Editor>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        cx.set_mention_source('@', source);
        cx.set_mention_source('#', entries);
    });
    let window = cx.add_window(|_, cx| {
        Editor::new("", cx).with_chrome(Chrome {
            mention,
            ..Chrome::default()
        })
    });
    let editor = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(480.0), px(400.0)));
    visual.update(|window, cx| {
        let handle = editor.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    visual.run_until_parked();
    (editor, visual)
}

fn source_of(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| editor.read(cx).source())
}

#[gpui::test]
fn enter_links_the_row_the_query_found(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(true, cx);
    cx.simulate_input("see @rel");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    assert_eq!(
        source_of(&editor, &mut cx),
        "see [app://release](app://release \"chip\")"
    );
}

#[gpui::test]
fn the_arrows_walk_the_rows(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(true, cx);
    cx.simulate_input("@");
    cx.run_until_parked();
    cx.simulate_keystrokes("down enter");
    assert_eq!(
        source_of(&editor, &mut cx),
        "[app://release](app://release \"chip\")"
    );
}

#[gpui::test]
fn an_editor_without_mentions_keeps_the_at(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(false, cx);
    cx.simulate_input("@rel");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    assert!(source_of(&editor, &mut cx).starts_with("@rel"));
}

#[gpui::test]
fn each_trigger_opens_its_own_source(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(true, cx);
    cx.simulate_input("see #12");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    assert_eq!(
        source_of(&editor, &mut cx),
        "see [app://entry/12](app://entry/12 \"chip\")"
    );
}

#[gpui::test]
fn a_character_with_no_source_opens_nothing(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(true, cx);
    cx.simulate_input("$rel");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    assert!(source_of(&editor, &mut cx).starts_with("$rel"));
}
