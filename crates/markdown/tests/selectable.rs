//! What a pointer selection copies out as.
//!
//! Not the editor's copy, which is `slice` then `serialize` and keeps the
//! markup (see `clipboard.rs`). This is the plain text a reader dragged over —
//! what a paste into anything else should read like.

use markdown::{BlockLayouts, Doc, parse::parse, selectable::copied, *};

fn body(block: usize, offset: usize) -> Cursor {
    Cursor::new(block, Part::Body, offset)
}

const DOC: &str = "# Title\n\nA paragraph with **bold** in it.\n\n- first\n- second";

#[test]
fn a_selection_inside_one_block_is_the_text_between_its_ends() {
    let doc = parse(DOC);
    let selection = Selection::new(body(1, 2), body(1, 11));
    assert_eq!(copied(&doc, selection), "paragraph");
}

#[test]
fn crossing_blocks_puts_a_newline_where_each_one_ended() {
    let doc = parse(DOC);
    let selection = Selection::new(body(2, 0), body(3, 6));
    assert_eq!(copied(&doc, selection), "first\nsecond");
}

#[test]
fn what_comes_out_is_text_rather_than_markup() {
    // The paragraph holds `**bold**`; a reader dragging over it selected the
    // word, not the asterisks — which is the whole difference from the
    // editor's own copy.
    let doc = parse(DOC);
    let whole = Selection::new(body(0, 0), body(1, 30));
    let text = copied(&doc, whole);
    assert!(
        !text.contains('*'),
        "marks are not part of the text: {text:?}"
    );
    assert!(text.starts_with("Title\n"), "{text:?}");
}

#[test]
fn a_press_that_never_dragged_copies_nothing() {
    let doc = parse(DOC);
    assert_eq!(copied(&doc, Selection::at(body(1, 4))), "");
}

#[gpui::test]
fn surface_takes_focus_and_copies_selected_plain_text(cx: &mut gpui::TestAppContext) {
    use gpui::{Context, Render, Window, prelude::*};
    struct Reader {
        focus: gpui::FocusHandle,
    }
    impl Render for Reader {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            selectable::surface(
                &self.focus,
                &parse("hello **world**"),
                Some(Selection::new(body(0, 6), body(0, 11))),
                cx,
            )
            .id("reader")
            .debug_selector(|| "reader".into())
            .size_full()
            .child("hello world")
        }
    }
    let window = cx.add_window(|_, cx| Reader {
        focus: cx.focus_handle(),
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    let point = visual.debug_bounds("reader").unwrap().center();
    visual.simulate_click(point, gpui::Modifiers::default());
    visual.run_until_parked();
    visual.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-c"
    } else {
        "ctrl-c"
    });
    visual.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .as_deref(),
            Some("world")
        )
    });
}

/// A drag that leaves the text keeps extending the selection.
///
/// `on_mouse_move` is delivered only over the element's hitbox, so without the
/// window listener the head would stop at whatever point was last inside —
/// several characters short of the edge on a fast drag, because moves are
/// sampled.
#[gpui::test]
fn a_drag_off_the_text_keeps_extending_the_selection(cx: &mut gpui::TestAppContext) {
    use gpui::{Context, Modifiers, MouseButton, Render, Window, div, prelude::*, px, size};

    const TEXT: &str = "a paragraph long enough to drag across and then leave";

    struct Reader {
        doc: Doc,
        layouts: BlockLayouts,
        selection: Option<Selection>,
        dragging: bool,
    }
    impl Render for Reader {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .id("page")
                .debug_selector(|| "page".into())
                .size_full()
                .pt(px(100.))
                .pl(px(100.))
                .child(selectable::render(
                    "reader",
                    &self.doc,
                    &self.layouts.clone(),
                    self.selection,
                    self.dragging,
                    window,
                    cx,
                    |view: &mut Reader, pointer, cx| {
                        match pointer {
                            selectable::Pointer::Down(cursor) => {
                                view.selection = Some(Selection::at(cursor));
                                view.dragging = true;
                            }
                            selectable::Pointer::Move(cursor) => {
                                view.selection =
                                    view.selection.map(|had| had.extend_to(cursor));
                            }
                            selectable::Pointer::Up => view.dragging = false,
                        }
                        cx.notify();
                    },
                ))
        }
    }

    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Reader {
        doc: parse(TEXT),
        layouts: BlockLayouts::default(),
        selection: None,
        dragging: false,
    });
    let reader = window.root(cx).unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(600.), px(400.)));
    visual.run_until_parked();

    // Press on the first line, then move the pointer well past the text on
    // both axes — the bottom right of a window the paragraph does not reach.
    let page = visual.debug_bounds("page").expect("the page painted");
    let start = gpui::point(page.left() + px(105.), page.top() + px(110.));
    visual.simulate_mouse_move(start, None, Modifiers::default());
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    visual.run_until_parked();
    assert!(
        visual.update(|_, cx| reader.read(cx).dragging),
        "the press started a drag"
    );

    visual.simulate_mouse_move(
        gpui::point(px(560.), px(360.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    visual.run_until_parked();

    let selection = visual
        .update(|_, cx| reader.read(cx).selection)
        .expect("the press left a selection");
    assert!(
        !selection.is_collapsed(),
        "a move off the text extends what the press started"
    );
    assert_eq!(
        selection.head,
        Cursor::new(0, Part::Body, TEXT.len()),
        "past the last line resolves to the end of the text"
    );
}
