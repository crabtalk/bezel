//! The editor driven through gpui's own harness — a real window, real key
//! dispatch, real layout.
//!
//! Everything else in this workspace is a pure function under test. That left
//! the half of the editor that needs a window with no tests at all, and it is
//! the half every bug so far has been in: a menu that never opened, rows that
//! did not answer a click, a caret that jumped at a block boundary. None of
//! those are reachable from a `Doc`.
//!
//! The test platform shapes text as a fixed-width font — `NoopTextSystem`
//! advances one em per glyph — so positions, wrapping and hit resolution are
//! all real. Column *values* here are therefore arbitrary; their relationships
//! are not, and the relationships are what broke.

use editor::Editor;
use gpui::{Entity, Focusable, TestAppContext, VisualTestContext, WindowHandle, px, size};

const SOURCE: &str = "# Title\n\nA paragraph long enough that it has to wrap more than once inside the pane it is painted into, which is what makes it worth testing.\n\n- first\n- second\n\n> a quote";

/// Open a focused editor in a drawn window.
fn open(cx: &mut TestAppContext) -> (Entity<Editor>, WindowHandle<Editor>, VisualTestContext) {
    cx.update(|cx| {
        // `appearance::init` asks AppKit what the system is set to, and there
        // is no NSApplication under the test platform. Installing the palette
        // directly is the same end state without the question.
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
    });
    let window = cx.add_window(|_, cx| Editor::new(SOURCE, cx));
    let editor = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    // Narrow enough that the paragraph wraps, which is the case the row-walk
    // has to get right.
    visual.simulate_resize(size(px(360.0), px(600.0)));
    visual.update(|window, cx| {
        let handle = editor.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    // The editor is the window's root view, so the window's own draw is what
    // fills the layouts every position here resolves against.
    visual.run_until_parked();
    (editor, window, visual)
}

fn head(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> markdown::Cursor {
    cx.update(|_, cx| editor.read(cx).selection().head)
}

fn source(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| editor.read(cx).source())
}

/// Walk the caret down until it reaches `block`, the way a reader would.
fn go_to_block(editor: &Entity<Editor>, cx: &mut VisualTestContext, block: usize) {
    for _ in 0..20 {
        if head(editor, cx).block == block {
            return;
        }
        cx.simulate_keystrokes("down");
    }
    panic!("never reached block {block}");
}

#[gpui::test]
fn typing_reaches_the_document(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open(cx);
    cx.simulate_input("Xy");
    assert!(
        source(&editor, &mut cx).starts_with("# XyTitle"),
        "typed text lands at the caret"
    );
}

#[gpui::test]
fn enter_splits_and_backspace_merges(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open(cx);
    cx.simulate_keystrokes("right right enter");
    assert!(
        source(&editor, &mut cx).starts_with("# Ti\n\ntle"),
        "a heading splits into a heading and body text"
    );
    cx.simulate_keystrokes("backspace");
    assert!(
        source(&editor, &mut cx).starts_with("# Title"),
        "and backspace at the seam puts it back"
    );
}

/// The bug: `Down` hit-tested a point one line below the caret, and the gap
/// between two blocks belongs to no run — so the nearest run was the one being
/// *left*, and the caret went sideways instead of down.
#[gpui::test]
fn down_crosses_every_block_boundary(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open(cx);
    let mut seen = vec![head(&editor, &mut cx)];
    for _ in 0..12 {
        cx.simulate_keystrokes("down");
        seen.push(head(&editor, &mut cx));
    }
    let blocks: Vec<usize> = seen.iter().map(|at| at.block).collect();
    assert!(
        blocks.windows(2).all(|pair| pair[1] >= pair[0]),
        "the caret never goes backwards: {blocks:?}"
    );
    assert_eq!(
        *blocks.last().unwrap(),
        4,
        "and it reaches the last block: {blocks:?}"
    );
}

/// The second half of the same bug: an offset at a soft wrap belongs to two
/// rows and resolves to the first, so a caret that re-derived its own row
/// stepped into the same one forever.
#[gpui::test]
fn down_does_not_stick_inside_a_wrapped_block(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open(cx);
    cx.simulate_keystrokes("down");
    let mut rows = Vec::new();
    for _ in 0..4 {
        cx.simulate_keystrokes("down");
        rows.push(head(&editor, &mut cx));
    }
    assert!(
        rows.windows(2).all(|pair| pair[0] != pair[1]),
        "every step moves: {rows:?}"
    );
}

#[gpui::test]
fn up_retraces_the_path_down(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open(cx);
    let start = head(&editor, &mut cx);
    cx.simulate_keystrokes("down down down");
    assert_ne!(head(&editor, &mut cx), start);
    cx.simulate_keystrokes("up up up");
    assert_eq!(
        head(&editor, &mut cx),
        start,
        "the goal column is held across the whole run"
    );
}

/// The bug: `render_with_selection` emptied the recorded layouts during *render*
/// and the menu read them after, so it never found the caret and never opened.
/// An open menu owns Enter, so what Enter *did* is the observable proof that
/// it opened — no accessor into the editor's insides required.
#[gpui::test]
fn the_menu_opens_on_a_slash_and_turns_the_block(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open(cx);
    // Onto an empty line below the heading, which is where a slash belongs.
    cx.simulate_keystrokes("end enter");
    cx.simulate_input("/");
    // One row past "Text", which turns a paragraph into a paragraph.
    cx.simulate_keystrokes("down enter");
    assert!(
        source(&editor, &mut cx).starts_with("# Title\n\n# "),
        "Enter took Heading 1 from the menu: {:?}",
        source(&editor, &mut cx)
    );
}

/// The state machine opening and the menu *painting* are two different things,
/// and the bug that shipped was the second one failing while the first looked
/// fine. Asserting on the painted frame is what tells them apart.
#[gpui::test]
fn the_menu_actually_paints(cx: &mut TestAppContext) {
    let (_editor, _window, mut cx) = open(cx);
    assert!(
        cx.debug_bounds(editor::SLASH_MENU).is_none(),
        "nothing is open yet"
    );
    cx.simulate_keystrokes("end enter");
    cx.simulate_input("/");
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(editor::SLASH_MENU).is_some(),
        "the menu reached the screen, not just the state"
    );
}

#[gpui::test]
fn escape_closes_the_menu_and_gives_enter_back(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open(cx);
    cx.simulate_keystrokes("end enter");
    cx.simulate_input("/");
    cx.simulate_keystrokes("escape enter");
    assert!(
        source(&editor, &mut cx).starts_with("# Title\n\n/"),
        "with the menu shut, Enter splits and the slash stays literal: {:?}",
        source(&editor, &mut cx)
    );
}

#[gpui::test]
fn a_selection_survives_a_mark_and_round_trips(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open(cx);
    cx.simulate_keystrokes("shift-right shift-right shift-right");
    assert!(
        !cx.update(|_, cx| editor.read(cx).selection().is_collapsed()),
        "shift+arrow extends"
    );
    cx.simulate_keystrokes("cmd-b");
    assert!(
        source(&editor, &mut cx).starts_with("# **Tit**le"),
        "cmd-B marks the selection: {:?}",
        source(&editor, &mut cx)
    );
}

#[gpui::test]
fn undo_gives_back_a_run_of_typing_at_once(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open(cx);
    let before = source(&editor, &mut cx);
    cx.simulate_input("hello");
    assert_ne!(source(&editor, &mut cx), before);
    cx.simulate_keystrokes("cmd-z");
    assert_eq!(
        source(&editor, &mut cx),
        before,
        "one step takes the whole word"
    );
}

#[gpui::test]
fn tab_indents_a_list_item_and_shift_tab_puts_it_back(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open(cx);
    // Into the second bullet, the only one with an item above it to nest under.
    go_to_block(&editor, &mut cx, 3);
    cx.simulate_keystrokes("tab");
    assert!(
        source(&editor, &mut cx).contains("- first\n    - second"),
        "tab nests it under the item above: {:?}",
        source(&editor, &mut cx)
    );
    cx.simulate_keystrokes("shift-tab");
    assert!(
        source(&editor, &mut cx).contains("- first\n- second"),
        "and shift-tab lifts it back"
    );
}
