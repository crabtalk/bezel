//! A field's right-click menu.

use gpui::{
    Context, Entity, Focusable, Modifiers, MouseButton, Render, TestAppContext, VisualTestContext,
    Window, div, prelude::*, px,
};
use theme::{Appearance, Theme};
use ui::{
    input::{self, TextField},
    menu::Edit,
};

struct Page(Entity<TextField>);

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(400.0)).child(self.0.clone())
    }
}

/// Right press the field and choose the row `selector` names off the menu
/// that opens.
fn choose(cx: &mut VisualTestContext, selector: &'static str) {
    // Inside the field, which is the page's first and only row.
    let at = gpui::point(px(20.0), px(15.0));
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::default());
    cx.run_until_parked();
    let row = cx.debug_bounds(selector).expect("the menu is open");
    cx.simulate_click(row.center(), Modifiers::default());
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
}

/// A focused field holding `hello`, painted once.
fn open(cx: &mut TestAppContext) -> (Entity<TextField>, VisualTestContext) {
    cx.update(|cx| {
        Theme::install(Appearance::Dark, cx);
        input::init(cx);
        ui::menu::init(cx);
    });
    let window = cx.add_window(|_, cx| Page(cx.new(TextField::new)));
    let page = window.root(cx).unwrap();
    let field = cx.update(|cx| page.read(cx).0.clone());
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.update(|window, cx| {
        window.focus(&field.read(cx).focus_handle(cx), cx);
        field.update(cx, |field, cx| field.set_content("hello", cx));
    });
    cx.run_until_parked();
    (field, cx)
}

#[gpui::test]
fn escape_closes_the_menu_and_hands_the_keys_back(cx: &mut TestAppContext) {
    let (field, mut cx) = open(cx);
    let at = gpui::point(px(20.0), px(15.0));
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::default());
    cx.run_until_parked();
    assert!(cx.debug_bounds("text-field-menu/Copy").is_some());
    cx.simulate_keystrokes("escape");
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    assert!(cx.debug_bounds("text-field-menu/Copy").is_none());
    cx.simulate_input("!");
    cx.update(|_, cx| assert!(field.read(cx).content().contains('!')));
}

#[gpui::test]
fn select_all_then_cut_off_the_menu_empties_the_field(cx: &mut TestAppContext) {
    let (field, mut cx) = open(cx);

    choose(&mut cx, "text-field-menu/Select All");
    choose(&mut cx, "text-field-menu/Cut");
    cx.update(|_, cx| {
        assert_eq!(field.read(cx).content().as_ref(), "");
        assert_eq!(
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .as_deref(),
            Some("hello")
        );
    });
}

#[test]
fn rows_name_the_edits_past_the_separator() {
    assert_eq!(Edit::at(&[0]), Some(Edit::Cut));
    assert_eq!(Edit::at(&[2]), Some(Edit::Paste));
    assert_eq!(Edit::at(&[3]), None);
    assert_eq!(Edit::at(&[4]), Some(Edit::SelectAll));
}
