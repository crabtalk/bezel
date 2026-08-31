//! Comment anchors under a real editor: the words a thread points at have to
//! stay the words it points at.
//!
//! Driven through key dispatch rather than by calling the mapping directly,
//! because the mapping is only half the claim — the other half is that every
//! path through `Editor::edit` reports what it did, and a path that forgets is
//! invisible to a unit test of the arithmetic.

use editor::{Anchor, CommentId, Editor};
use gpui::{Entity, Focusable, TestAppContext, VisualTestContext, px, size};
use markdown::{Cursor, Part, Selection};

/// Three paragraphs, so there is something above an anchor to reorder and
/// something below it to shift.
const SOURCE: &str = "alpha one\n\nbravo two\n\ncharlie three";

const ID: CommentId = CommentId(1);

/// The primary modifier, which `editor::keys` splits the keymap on: cmd on
/// macOS, ctrl everywhere else. A test that names one chord outright passes on
/// one platform and silently does nothing on the other.
#[cfg(target_os = "macos")]
const PRIMARY: &str = "cmd";
#[cfg(not(target_os = "macos"))]
const PRIMARY: &str = "ctrl";

fn open(cx: &mut TestAppContext) -> (Entity<Editor>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
    });
    let window = cx.add_window(|_, cx| Editor::new(SOURCE, cx));
    let editor = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(360.0), px(600.0)));
    visual.update(|window, cx| {
        let handle = editor.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    visual.run_until_parked();
    (editor, visual)
}

fn at(block: usize, offset: usize) -> Cursor {
    Cursor::new(block, Part::Body, offset)
}

/// Anchor `block`'s `from..to`, and put the caret where the test wants to type.
fn anchor(
    editor: &Entity<Editor>,
    cx: &mut VisualTestContext,
    range: (usize, usize, usize),
    caret: Cursor,
) {
    let (block, from, to) = range;
    cx.update(|_, cx| {
        editor.update(cx, |editor, cx| {
            editor.set_anchors(
                vec![Anchor::new(
                    ID,
                    Selection::new(at(block, from), at(block, to)),
                )],
                cx,
            );
            editor.select(Selection::at(caret), cx);
        })
    });
}

/// The anchored range, as `(block, start, end)`.
fn anchored(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> (usize, usize, usize) {
    cx.update(|_, cx| {
        let anchor = &editor.read(cx).anchors()[0];
        let (start, end) = anchor.range.ordered();
        (start.block, start.offset, end.offset)
    })
}

fn detached(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| editor.read(cx).anchors()[0].detached())
}

/// "one" in block 0, which is `alpha one` — offsets 6..9.
const ONE: (usize, usize, usize) = (0, 6, 9);

#[gpui::test]
fn typing_before_the_range_carries_it_along(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(cx);
    anchor(&editor, &mut cx, ONE, at(0, 0));
    cx.simulate_input("XX");
    assert_eq!(
        anchored(&editor, &mut cx),
        (0, 8, 11),
        "both ends move by what went in ahead of them"
    );
}

#[gpui::test]
fn typing_inside_the_range_widens_it(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(cx);
    anchor(&editor, &mut cx, ONE, at(0, 7));
    cx.simulate_input("XX");
    assert_eq!(
        anchored(&editor, &mut cx),
        (0, 6, 11),
        "the start holds and the end gives way"
    );
}

/// The left-sticky rule the marks already follow: typing at the end of a run
/// joins it, typing at the start does not.
#[gpui::test]
fn typing_at_the_end_joins_the_comment(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(cx);
    anchor(&editor, &mut cx, ONE, at(0, 9));
    cx.simulate_input("X");
    assert_eq!(anchored(&editor, &mut cx), (0, 6, 10));
}

#[gpui::test]
fn typing_at_the_start_stays_outside_it(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(cx);
    anchor(&editor, &mut cx, ONE, at(0, 6));
    cx.simulate_input("X");
    assert_eq!(anchored(&editor, &mut cx), (0, 7, 10));
}

#[gpui::test]
fn typing_after_the_range_leaves_it_alone(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(cx);
    anchor(&editor, &mut cx, (0, 0, 5), at(0, 9));
    cx.simulate_input("XX");
    assert_eq!(anchored(&editor, &mut cx), (0, 0, 5));
}

#[gpui::test]
fn typing_in_a_block_above_moves_nothing(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(cx);
    anchor(&editor, &mut cx, (1, 0, 5), at(0, 0));
    cx.simulate_input("XX");
    assert_eq!(
        anchored(&editor, &mut cx),
        (1, 0, 5),
        "an offset in another block is not an offset in this one"
    );
}

/// Enter above an anchor pushes its whole block down, which is the shift a
/// splice's block count carries.
#[gpui::test]
fn splitting_a_block_above_shifts_the_anchor_down(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(cx);
    anchor(&editor, &mut cx, (1, 0, 5), at(0, 5));
    cx.simulate_keystrokes("enter");
    assert_eq!(
        anchored(&editor, &mut cx),
        (2, 0, 5),
        "the anchored block is one further down"
    );
}

#[gpui::test]
fn reordering_the_block_above_carries_the_anchor(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(cx);
    anchor(&editor, &mut cx, ONE, at(0, 0));
    // Move the anchored block itself down past its neighbour.
    cx.update(|_, cx| editor.update(cx, |editor, cx| editor.move_block(0, 1, cx)));
    assert_eq!(
        anchored(&editor, &mut cx),
        (1, 6, 9),
        "the anchor rides the block it was on"
    );
}

#[gpui::test]
fn removing_a_block_above_pulls_the_anchor_up(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(cx);
    anchor(&editor, &mut cx, (1, 0, 5), at(0, 0));
    cx.update(|_, cx| editor.update(cx, |editor, cx| editor.remove_block(0, cx)));
    assert_eq!(anchored(&editor, &mut cx), (0, 0, 5));
}

#[gpui::test]
fn removing_the_anchored_block_detaches_it(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(cx);
    anchor(&editor, &mut cx, ONE, at(0, 0));
    cx.update(|_, cx| editor.update(cx, |editor, cx| editor.remove_block(0, cx)));
    assert!(
        detached(&editor, &mut cx),
        "the words are gone, and the thread is kept to say so"
    );
}

#[gpui::test]
fn deleting_the_anchored_words_detaches_it(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(cx);
    // Select exactly the anchored word and delete it.
    anchor(&editor, &mut cx, ONE, at(0, 6));
    cx.update(|_, cx| {
        editor.update(cx, |editor, cx| {
            editor.select(Selection::new(at(0, 6), at(0, 9)), cx)
        })
    });
    cx.simulate_keystrokes("backspace");
    assert!(detached(&editor, &mut cx));
}

/// The reason the anchors live in the editor rather than in the app: an undo
/// restores a whole document, and there is no delta to map through.
#[gpui::test]
fn undo_and_redo_restore_the_anchor_with_the_document(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(cx);
    anchor(&editor, &mut cx, ONE, at(0, 0));
    cx.simulate_input("XX");
    assert_eq!(anchored(&editor, &mut cx), (0, 8, 11));

    cx.simulate_keystrokes(&format!("{PRIMARY}-z"));
    assert_eq!(
        anchored(&editor, &mut cx),
        ONE,
        "the anchor of that moment came back with the document"
    );

    cx.simulate_keystrokes(&format!("{PRIMARY}-shift-z"));
    assert_eq!(
        anchored(&editor, &mut cx),
        (0, 8, 11),
        "and redo replays it"
    );
}

#[gpui::test]
fn a_click_finds_the_comment_under_it(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(cx);
    anchor(&editor, &mut cx, ONE, at(0, 0));
    let bounds = cx
        .update(|_, cx| editor.read(cx).anchor_bounds(ID))
        .expect("the anchor painted somewhere");
    assert_eq!(
        cx.update(|_, cx| editor.read(cx).comment_at(bounds.origin)),
        Some(ID),
        "a point on the range answers with its thread"
    );
}
