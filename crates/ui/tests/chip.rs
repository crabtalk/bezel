//! Chips in a field: a range of the content painted as a glyph and a title,
//! and edited as one unit.

use gpui::{
    Context, Entity, EntityInputHandler, Focusable, Render, TestAppContext, VisualTestContext,
    Window, div, prelude::*, px,
};
use theme::{Appearance, Theme};
use ui::{
    icons::{Icon, glyph},
    input::{self, Chip, Shown, TextField},
};

const LINK: &str = "[x](app://x \"chip\")";

struct Page(Entity<TextField>);

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(400.0)).child(self.0.clone())
    }
}

fn chip(range: std::ops::Range<usize>) -> Chip {
    Chip {
        range,
        glyph: Icon::glyph(glyph::X),
        title: "Release".into(),
    }
}

/// A focused field holding `a <LINK> b`, the link a chip, painted once.
fn open(cx: &mut TestAppContext) -> (Entity<TextField>, VisualTestContext) {
    cx.update(|cx| {
        Theme::install(Appearance::Dark, cx);
        input::init(cx);
    });
    let window = cx.add_window(|_, cx| Page(cx.new(TextField::new)));
    let page = window.root(cx).unwrap();
    let field = cx.update(|cx| page.read(cx).0.clone());
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| {
        window.focus(&field.read(cx).focus_handle(cx), cx);
        field.update(cx, |field, cx| {
            field.set_content(format!("a {LINK} b"), cx);
            field.set_chips(vec![chip(2..2 + LINK.len())], cx);
        });
    });
    visual.run_until_parked();
    (field, visual)
}

fn read<T>(
    field: &Entity<TextField>,
    cx: &mut VisualTestContext,
    f: impl FnOnce(&TextField) -> T,
) -> T {
    cx.update(|_, cx| f(field.read(cx)))
}

#[test]
fn shown_offsets_land_on_an_atoms_ends() {
    let shown = Shown::new(vec![(2..10, 2..5)]);
    assert_eq!(shown.at(1), 1);
    assert_eq!(shown.at(4), 2, "inside lands on the start");
    assert_eq!(shown.at(12), 7, "after moves by what the atom lost");
    assert_eq!(shown.range(&(4..6)), 2..5, "an end inside takes it all");
    assert_eq!(shown.offset(3), 2, "nearer the start");
    assert_eq!(shown.offset(4), 10, "nearer the end");
    assert_eq!(shown.offset(7), 12);
}

#[gpui::test]
fn the_caret_steps_over_a_chip(cx: &mut TestAppContext) {
    let (field, mut cx) = open(cx);
    let end = 2 + LINK.len();
    cx.update(|_, cx| field.update(cx, |field, cx| field.select(end..end, cx)));
    cx.simulate_keystrokes("left");
    assert_eq!(read(&field, &mut cx, |field| field.cursor()), 2);
    cx.simulate_keystrokes("right");
    assert_eq!(read(&field, &mut cx, |field| field.cursor()), end);
    cx.simulate_keystrokes("shift-left");
    assert_eq!(
        read(&field, &mut cx, |field| field.cursor()),
        2,
        "a selection takes the chip whole"
    );
}

#[gpui::test]
fn a_selection_cutting_a_chip_takes_all_of_it(cx: &mut TestAppContext) {
    let (field, mut cx) = open(cx);
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.select(0..5, cx);
            let range = field.selected_text_range(false, window, cx).unwrap().range;
            assert_eq!(range, 0..2 + LINK.len());
            field.select(5..5, cx);
            assert_eq!(
                field.cursor(),
                2 + LINK.len(),
                "a caret inside goes to the end"
            );
        })
    });
}

#[gpui::test]
fn one_backspace_removes_a_chip_and_undo_brings_it_back(cx: &mut TestAppContext) {
    let (field, mut cx) = open(cx);
    let end = 2 + LINK.len();
    cx.update(|_, cx| field.update(cx, |field, cx| field.select(end..end, cx)));
    cx.simulate_keystrokes("backspace");
    assert_eq!(
        read(&field, &mut cx, |field| field.content().to_string()),
        "a  b"
    );
    assert!(read(&field, &mut cx, |field| field.chips().is_empty()));
    cx.dispatch_action(input::Undo);
    assert_eq!(
        read(&field, &mut cx, |field| field.content().to_string()),
        format!("a {LINK} b")
    );
    assert_eq!(
        read(&field, &mut cx, |field| field.chips().to_vec()),
        vec![chip(2..end)]
    );
}

#[gpui::test]
fn chips_follow_edits_around_them(cx: &mut TestAppContext) {
    let (field, mut cx) = open(cx);
    let end = 2 + LINK.len();
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.select(2..2, cx);
            field.replace_text_in_range(None, "zz", window, cx);
            assert_eq!(field.chips()[0].range, 4..end + 2, "typing before moves it");
            field.select(end + 2..end + 2, cx);
            field.replace_text_in_range(None, "y", window, cx);
            assert_eq!(field.chips()[0].range, 4..end + 2, "typing after leaves it");
            // An explicit range is how an edit lands inside one.
            field.replace_text_in_range(Some(6..7), "q", window, cx);
            assert!(field.chips().is_empty(), "an edit inside drops it");
        })
    });
}

/// A painted chip shows its title, not its source, so the field is narrower
/// than the source would be.
#[gpui::test]
fn a_chip_shows_its_title(cx: &mut TestAppContext) {
    let (field, mut cx) = open(cx);
    let end = 2 + LINK.len();
    let after = cx.update(|_, cx| field.read(cx).offset_bounds(end).unwrap().origin.x);
    let before = cx.update(|_, cx| field.read(cx).offset_bounds(2).unwrap().origin.x);
    cx.update(|_, cx| field.update(cx, |field, cx| field.set_chips(Vec::new(), cx)));
    cx.run_until_parked();
    let source = cx.update(|_, cx| field.read(cx).offset_bounds(end).unwrap().origin.x);
    assert!(after > before);
    assert!(source > after, "the source is wider than `Release`");
}
