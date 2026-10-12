//! Two editors on one document: one text, a caret each.

use editor::{Document, Editor, Mode};
use gpui::{
    AppContext as _, Context, Entity, Focusable, IntoElement, Render, TestAppContext,
    VisualTestContext, Window, div, prelude::*,
};
use markdown::{Cursor, Part, Selection};

struct Pair {
    left: Entity<Editor>,
    right: Entity<Editor>,
}

impl Render for Pair {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .child(div().flex_1().child(self.left.clone()))
            .child(div().flex_1().child(self.right.clone()))
    }
}

fn open(
    source: &str,
    cx: &mut TestAppContext,
) -> (Entity<Editor>, Entity<Editor>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
    });
    let window = cx.add_window(|_, cx| {
        let document = cx.new(|cx| Document::new(source, cx));
        Pair {
            left: cx.new(|cx| Editor::for_document(document.clone(), cx)),
            right: cx.new(|cx| Editor::for_document(document, cx)),
        }
    });
    let pair = window.root(cx).unwrap();
    let (left, right) = cx.update(|cx| {
        let pair = pair.read(cx);
        (pair.left.clone(), pair.right.clone())
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| window.focus(&left.read(cx).focus_handle(cx), cx));
    visual.run_until_parked();
    (left, right, visual)
}

fn at(offset: usize) -> Selection {
    Selection::at(Cursor::new(0, Part::Body, offset))
}

#[gpui::test]
fn typing_in_one_editor_shows_in_the_other(cx: &mut TestAppContext) {
    let (left, right, mut cx) = open("alpha", cx);
    cx.update(|_, cx| {
        left.update(cx, |editor, cx| editor.select(at(0), cx));
        right.update(cx, |editor, cx| editor.select(at(5), cx));
    });
    cx.simulate_input("X");
    cx.run_until_parked();

    assert_eq!(cx.update(|_, cx| right.read(cx).source()), "Xalpha");
    assert_eq!(
        cx.update(|_, cx| right.read(cx).selection()),
        at(6),
        "the caret behind the edit is carried past it"
    );
    assert_eq!(cx.update(|_, cx| left.read(cx).selection()), at(1));
}

#[gpui::test]
fn either_editor_undoes_the_shared_history(cx: &mut TestAppContext) {
    let (left, right, mut cx) = open("alpha", cx);
    cx.update(|_, cx| left.update(cx, |editor, cx| editor.select(at(5), cx)));
    cx.simulate_input(" beta");
    cx.run_until_parked();
    cx.update(|window, cx| window.focus(&right.read(cx).focus_handle(cx), cx));
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-z"
    } else {
        "ctrl-z"
    });
    cx.run_until_parked();

    assert_eq!(cx.update(|_, cx| left.read(cx).source()), "alpha");
    assert_eq!(cx.update(|_, cx| right.read(cx).source()), "alpha");
}

#[gpui::test]
fn a_switch_of_form_reaches_every_editor(cx: &mut TestAppContext) {
    let (left, right, mut cx) = open("# Title\n\nbody", cx);
    cx.update(|_, cx| left.update(cx, |editor, cx| editor.set_mode(Mode::Source, cx)));
    cx.run_until_parked();

    assert_eq!(cx.update(|_, cx| right.read(cx).mode()), Mode::Source);
    assert_eq!(
        cx.update(|_, cx| right.read(cx).source()),
        "# Title\n\nbody"
    );
}

#[gpui::test]
fn an_edit_and_a_switch_leave_the_other_blocks_bytes(cx: &mut TestAppContext) {
    let (left, right, mut cx) = open("*  first\n\nSetext\n======\n\nlast\n", cx);
    cx.update(|_, cx| {
        left.update(cx, |editor, cx| {
            editor.select(Selection::at(Cursor::new(2, Part::Body, 4)), cx)
        })
    });
    cx.simulate_input("!");
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| right.read(cx).source()),
        "*  first\n\nSetext\n======\n\nlast!\n"
    );

    cx.update(|_, cx| left.update(cx, |editor, cx| editor.set_mode(Mode::Source, cx)));
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| right.read(cx).source()),
        "*  first\n\nSetext\n======\n\nlast!\n"
    );
}
