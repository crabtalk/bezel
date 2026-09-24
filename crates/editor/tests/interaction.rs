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

use std::sync::Mutex;

use editor::{Editor, ImageStore, Source};
use gpui::{
    App, ClipboardEntry, ClipboardItem, ClipboardString, Context, Entity, EntityId, ExternalPaths,
    Focusable, Render, ScrollHandle, TestAppContext, VisualTestContext, Window, WindowHandle,
    point, prelude::*, px, size,
};

const SOURCE: &str = "# Title\n\nA paragraph long enough that it has to wrap more than once inside the pane it is painted into, which is what makes it worth testing.\n\n- first\n- second\n\n> a quote";

/// Open a focused editor in a drawn window.
fn open(cx: &mut TestAppContext) -> (Entity<Editor>, WindowHandle<Editor>, VisualTestContext) {
    open_with(SOURCE, cx)
}

fn open_with(
    source: &str,
    cx: &mut TestAppContext,
) -> (Entity<Editor>, WindowHandle<Editor>, VisualTestContext) {
    cx.update(|cx| {
        // `appearance::init` asks AppKit what the system is set to, and there
        // is no NSApplication under the test platform. Installing the palette
        // directly is the same end state without the question.
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
    });
    let window = cx.add_window(|_, cx| Editor::new(source, cx));
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

struct ScrollingEditor {
    editor: Entity<Editor>,
    scroll: ScrollHandle,
}

impl Render for ScrollingEditor {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        ui::scroll::pane("editor-test", ui::scroll::Axes::Vertical)
            .size_full()
            .track_scroll(&self.scroll)
            .child(self.editor.clone())
    }
}

fn open_scrolling_with(
    source: &str,
    cx: &mut TestAppContext,
) -> (Entity<Editor>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
    });
    let window = cx.add_window(|_, cx| {
        let scroll = ScrollHandle::new();
        let editor = cx.new({
            let scroll = scroll.clone();
            |cx| Editor::new(source, cx).with_scroll(scroll)
        });
        ScrollingEditor { editor, scroll }
    });
    let host = window.root(cx).unwrap();
    let editor = cx.update(|cx| host.read(cx).editor.clone());
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(360.0), px(70.0)));
    visual.update(|window, cx| {
        let handle = editor.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    visual.run_until_parked();
    (editor, visual)
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

#[gpui::test]
fn shift_enter_inserts_a_soft_newline(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("hello", cx);
    cx.simulate_keystrokes("right right shift-enter");
    assert_eq!(source(&editor, &mut cx), "he\nllo");
}

#[gpui::test]
fn ctrl_enter_inserts_a_paragraph_after_the_current_block(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("one\n\ntwo", cx);
    cx.simulate_keystrokes("end ctrl-enter");
    cx.simulate_input("middle");
    assert_eq!(source(&editor, &mut cx), "one\n\nmiddle\n\ntwo");
}

#[gpui::test]
fn enter_after_a_quote_opens_plain_text(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("> quote", cx);
    cx.simulate_keystrokes("end enter");
    cx.simulate_input("plain");
    assert_eq!(source(&editor, &mut cx), "> quote\n\nplain");
}

/// Leaving a quote is what Enter at the *end* of one means. In the middle of a
/// sentence it is still a split, and half a quote is not plain text.
#[gpui::test]
fn enter_inside_a_quote_keeps_both_halves_quoted(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("> hello world", cx);
    cx.simulate_keystrokes("home right right right right right enter");
    assert_eq!(source(&editor, &mut cx), "> hello\n\n> world");
}

#[gpui::test]
fn enter_inside_an_alert_keeps_the_alert(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("> [!NOTE]\n> hello world", cx);
    cx.simulate_keystrokes("home right right right right right enter");
    assert_eq!(
        source(&editor, &mut cx),
        "> [!NOTE]\n> hello\n\n> [!NOTE]\n> world"
    );
}

#[gpui::test]
fn the_slash_menu_owns_every_enter_chord(cx: &mut TestAppContext) {
    for chord in ["enter", "shift-enter", "ctrl-enter"] {
        let (editor, _window, mut cx) = open_with("", cx);
        cx.simulate_input("/note");
        cx.simulate_keystrokes(chord);
        assert_eq!(
            source(&editor, &mut cx),
            "> [!NOTE]",
            "{chord} picked from the menu instead of editing under it"
        );
    }
}

/// The marker is a first line, not the whole quote: what is already written
/// under it becomes the alert's body rather than going away with the marker.
#[gpui::test]
fn a_marker_typed_above_a_body_promotes_and_keeps_it(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("", cx);
    cx.simulate_input("> ");
    cx.simulate_input("body");
    cx.simulate_keystrokes("home");
    cx.simulate_input("[!NOTE]");
    cx.simulate_keystrokes("shift-enter");
    assert_eq!(source(&editor, &mut cx), "> [!NOTE]\n> body");
    cx.simulate_input("X");
    assert_eq!(source(&editor, &mut cx), "> [!NOTE]\n> Xbody");
}

/// Ctrl+Enter asks for a paragraph after *this* block, so in the middle of a
/// list that is where it goes and the list is two lists.
#[gpui::test]
fn ctrl_enter_inside_a_list_breaks_it_in_two(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("- one\n- two", cx);
    cx.simulate_keystrokes("end ctrl-enter");
    cx.simulate_input("mid");
    assert_eq!(source(&editor, &mut cx), "- one\n\nmid\n\n- two");
}

#[gpui::test]
fn typing_an_alert_marker_after_quote_shortcut_promotes_it(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("", cx);
    cx.simulate_input("> ");
    cx.simulate_input("[!NOTE]");
    cx.simulate_keystrokes("shift-enter");
    cx.simulate_input("body");
    assert_eq!(source(&editor, &mut cx), "> [!NOTE]\n> body");
}

#[gpui::test]
fn the_slash_menu_offers_every_gfm_alert_quote(cx: &mut TestAppContext) {
    for (query, expected) in [
        ("note", "> [!NOTE]"),
        ("tip", "> [!TIP]"),
        ("important", "> [!IMPORTANT]"),
        ("warning", "> [!WARNING]"),
        ("caution", "> [!CAUTION]"),
    ] {
        let (editor, _window, mut cx) = open_with("", cx);
        cx.simulate_input("/");
        cx.simulate_input(query);
        cx.simulate_keystrokes("enter");
        assert_eq!(
            source(&editor, &mut cx),
            expected,
            "{query} picked the right alert"
        );
    }
}

#[gpui::test]
fn home_stays_on_the_softbreak_line(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("ab\ncd", cx);
    cx.simulate_keystrokes("right right right home");
    cx.simulate_input("X");
    assert_eq!(source(&editor, &mut cx), "ab\nXcd");
}

#[gpui::test]
fn end_stays_on_the_softbreak_line(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("ab\ncd", cx);
    cx.simulate_keystrokes("end");
    cx.simulate_input("X");
    assert_eq!(source(&editor, &mut cx), "abX\ncd");
}

#[gpui::test]
fn down_moves_to_the_second_softbreak_line(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("ab\ncd", cx);
    cx.simulate_keystrokes("down");
    cx.simulate_input("X");
    assert_eq!(source(&editor, &mut cx), "ab\nXcd");
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

#[gpui::test]
fn vertical_motion_keeps_its_row_while_scrolling(cx: &mut TestAppContext) {
    let source = "word ".repeat(100);
    let (editor, mut cx) = open_scrolling_with(&source, cx);
    cx.simulate_keystrokes("down down");
    let before = head(&editor, &mut cx);
    let before_y = cx.update(|_, cx| {
        let editor = editor.read(cx);
        editor
            .layouts()
            .position(editor.selection().head)
            .unwrap()
            .0
            .y
    });
    cx.simulate_event(gpui::ScrollWheelEvent {
        position: point(px(100.0), px(35.0)),
        delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(-35.0))),
        modifiers: Default::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
    cx.run_until_parked();
    let after_y = cx.update(|_, cx| {
        let editor = editor.read(cx);
        editor
            .layouts()
            .position(editor.selection().head)
            .unwrap()
            .0
            .y
    });
    assert_ne!(before_y, after_y, "the test must scroll the painted rows");
    cx.simulate_keystrokes("down");
    assert!(head(&editor, &mut cx) > before);
}

#[gpui::test]
fn typing_after_vertical_motion_sets_a_new_goal_column(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("abcdefghij\nx\nabcdefghij", cx);
    cx.simulate_keystrokes("right right right right right right right down");
    assert_eq!(head(&editor, &mut cx).offset, 12);
    cx.simulate_input("y");
    assert_eq!(source(&editor, &mut cx), "abcdefghij\nxy\nabcdefghij");
    assert_eq!(head(&editor, &mut cx).offset, 13);
    cx.simulate_keystrokes("down");
    assert_eq!(head(&editor, &mut cx).offset, 16);
}

#[gpui::test]
fn vertical_motion_chooses_the_nearest_column(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("xy\nabcdefghij", cx);
    cx.simulate_keystrokes("right right down");
    assert_eq!(head(&editor, &mut cx).offset, 5);
}

/// Document start, document end and select-to-end, as each keymap spells them.
#[cfg(target_os = "macos")]
const DOCUMENT_ENDS: [&str; 3] = ["cmd-up", "cmd-down", "cmd-shift-down"];
#[cfg(not(target_os = "macos"))]
const DOCUMENT_ENDS: [&str; 3] = ["ctrl-home", "ctrl-end", "ctrl-shift-end"];

#[gpui::test]
fn chords_move_and_select_to_document_ends(cx: &mut TestAppContext) {
    let [start, end, select_end] = DOCUMENT_ENDS;
    let (editor, _window, mut cx) = open_with("first\n\nsecond\n\nthird", cx);
    cx.simulate_keystrokes(end);
    assert_eq!(head(&editor, &mut cx).block, 2);
    assert_eq!(head(&editor, &mut cx).offset, 5);
    cx.simulate_keystrokes(start);
    assert_eq!(head(&editor, &mut cx).block, 0);
    assert_eq!(head(&editor, &mut cx).offset, 0);
    cx.simulate_keystrokes(select_end);
    let selection = cx.update(|_, cx| editor.read(cx).selection());
    assert_eq!(selection.anchor.block, 0);
    assert_eq!(selection.head.block, 2);
    assert_eq!(selection.head.offset, 5);
}

/// Off either end there is no row to step to, and the column survives the
/// trip: coming back lands where the walk started, not under the end.
#[gpui::test]
fn the_goal_column_survives_both_ends(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("abcdefghij\nabcdefghij\nabcdefghij", cx);
    cx.simulate_keystrokes("right right right right right down down down up");
    assert_eq!(head(&editor, &mut cx).offset, 16);
    cx.simulate_keystrokes("down up up up down");
    assert_eq!(head(&editor, &mut cx).offset, 16);
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

/// The primary modifier, which `editor::keys` splits the keymap on: cmd on
/// macOS, ctrl everywhere else. A test that names one chord outright passes on
/// one platform and silently does nothing on the other.
#[cfg(target_os = "macos")]
const PRIMARY: &str = "cmd";
#[cfg(not(target_os = "macos"))]
const PRIMARY: &str = "ctrl";

#[gpui::test]
fn a_selection_survives_a_mark_and_round_trips(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open(cx);
    cx.simulate_keystrokes("shift-right shift-right shift-right");
    assert!(
        !cx.update(|_, cx| editor.read(cx).selection().is_collapsed()),
        "shift+arrow extends"
    );
    cx.simulate_keystrokes(&format!("{PRIMARY}-b"));
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
    cx.simulate_keystrokes(&format!("{PRIMARY}-z"));
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

/// The bug (#14): `tab` is bound twice — `Indent` here, `FocusNext` in
/// [`ui::focus`] with no context and so at the same depth — and the tie went to
/// whichever crate was initialised last. In the gallery that was `focus`, so
/// `tab` moved focus and a list could not be nested at all.
#[gpui::test]
fn tab_indents_with_focus_traversal_installed(cx: &mut TestAppContext) {
    use gpui::{Context, IntoElement, Render, Window, div, prelude::*};

    /// A host that traverses on `tab`, as an app's root view does.
    struct Host(Entity<Editor>);

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            ui::focus::traversal(div())
                .size_full()
                .child(self.0.clone())
        }
    }

    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        // The order `apps/gallery` installs them in: `focus` binds `tab` last.
        ui::focus::init(cx);
    });
    let window = cx.add_window(|_, cx| Host(cx.new(|cx| Editor::new(SOURCE, cx))));
    let editor = cx.update(|cx| window.root(cx).unwrap().read(cx).0.clone());
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(360.0), px(600.0)));
    cx.update(|window, cx| {
        let handle = editor.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    cx.run_until_parked();

    go_to_block(&editor, &mut cx, 3);
    cx.simulate_keystrokes("tab");
    assert!(
        source(&editor, &mut cx).contains("- first\n    - second"),
        "tab nests the item rather than moving focus: {:?}",
        source(&editor, &mut cx)
    );
    cx.simulate_keystrokes("shift-tab");
    assert!(
        source(&editor, &mut cx).contains("- first\n- second"),
        "and shift-tab lifts it back rather than stepping the other way"
    );
    assert!(
        cx.update(|window, cx| editor.read(cx).focus_handle(cx).is_focused(window)),
        "the document still holds focus"
    );
}

/// The handle used to follow the pointer and nothing else, so a document being
/// worked in by keyboard had no handle at all until you reached for the mouse.
#[gpui::test]
fn the_handle_follows_the_caret(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open(cx);
    let at = cx
        .debug_bounds(editor::BLOCK_HANDLE)
        .expect("a focused document shows a handle without being hovered");

    go_to_block(&editor, &mut cx, 2);
    cx.run_until_parked();
    let moved = cx
        .debug_bounds(editor::BLOCK_HANDLE)
        .expect("and still shows one");
    assert!(
        moved.origin.y > at.origin.y,
        "it followed the caret down the document"
    );
}

/// The bug (#14): the block's box was recorded outside its indent padding, so
/// every level answered with the same left edge and the handle stayed at the
/// margin while the block it belongs to moved right. And because the handle is
/// built from the frame before's records, the frame that would put it right
/// was only ever the *next* one somebody else asked for — the caret blink,
/// half a second later.
#[gpui::test]
fn the_handle_moves_in_with_an_indented_block(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("- first\n- second", &mut *cx);
    go_to_block(&editor, &mut cx, 1);
    cx.run_until_parked();
    let before = cx
        .debug_bounds(editor::BLOCK_HANDLE)
        .expect("a handle")
        .origin;

    cx.simulate_keystrokes("tab");
    // The frame the editor asks for once it sees the block has moved out from
    // under the handle. A running app draws it; a test has to say so.
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();

    let after = cx
        .debug_bounds(editor::BLOCK_HANDLE)
        .expect("still a handle")
        .origin;
    assert_eq!(
        after.x - before.x,
        px(22.0),
        "the handle followed the block in by one indent"
    );
    assert_eq!(after.y, before.y, "and stayed on the same row");
}

/// The bug: backspace at the start of an empty block steps the caret into the
/// previous block's last part — right while the block still holds text, and a
/// trap once it does not. Nothing above an atomic block merges, so the empty
/// one was left behind with no way left to reach it.
#[gpui::test]
fn an_empty_block_after_an_atomic_one_deletes(cx: &mut TestAppContext) {
    for (name, source) in [
        ("a rule", "a\n\n---\n\nx"),
        ("a fence", "a\n\n```rs\nk\n```\n\nx"),
        ("an image", "a\n\n![c](https://e.com/i.png)\n\nx"),
        ("a table", "a\n\n| h |\n| - |\n| c |\n\nx"),
    ] {
        let (editor, _window, mut cx) = open_with(source, &mut *cx);
        // Empty the trailing paragraph, then try to take the paragraph itself.
        for _ in 0..25 {
            cx.simulate_keystrokes("down");
        }
        cx.simulate_keystrokes("end backspace");
        let before = cx.update(|_, cx| editor.read(cx).doc().blocks.len());
        cx.simulate_keystrokes("backspace");
        assert_eq!(
            cx.update(|_, cx| editor.read(cx).doc().blocks.len()),
            before - 1,
            "an empty paragraph after {name} is deletable"
        );
    }
}

/// The press that dismisses a menu is the same press that would reopen it: the
/// card's `on_mouse_down_out` fires on mouse-DOWN, the handle's click on
/// mouse-UP. Without the note taken on the way down, the second press closes
/// and the release opens it straight back up (user report).
#[gpui::test]
fn a_second_press_on_the_handle_leaves_the_block_menu_shut(cx: &mut TestAppContext) {
    let (_editor, _window, mut cx) = open(cx);
    cx.run_until_parked();
    let handle = cx
        .debug_bounds(editor::BLOCK_HANDLE)
        .expect("the focused caret's block paints a handle");

    // Two different corners of the same 18px handle: the menu hangs its own
    // top-left off wherever the press landed, so pressing the second time
    // further up-left is what keeps the press on the handle and off the card.
    let press = handle.origin + gpui::point(px(15.0), px(15.0));
    let press_again = handle.origin + gpui::point(px(3.0), px(3.0));

    cx.simulate_click(press, gpui::Modifiers::default());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(editor::BLOCK_MENU).is_some(),
        "the first press opens it"
    );

    cx.simulate_click(press_again, gpui::Modifiers::default());
    // Past the exit animation, so what is left is what stayed rather than what
    // is still fading.
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(editor::BLOCK_MENU).is_none(),
        "the second press leaves it shut instead of closing and reopening"
    );
}

/// Copying a file in a file manager rather than dragging it. macOS puts the
/// path on the clipboard as text beside the file itself, and the text is not
/// the picture — which is the whole of the bug this answers.
#[gpui::test]
fn a_copied_image_file_pastes_as_the_picture(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("", cx);
    // A space in the path, because a project directory has one and the
    // destination has to come back through the serializer intact.
    let path = std::path::PathBuf::from("/My Notes/shot.png");
    cx.update(|_, cx| {
        cx.write_to_clipboard(ClipboardItem {
            entries: vec![
                ClipboardEntry::ExternalPaths(ExternalPaths(vec![path.clone()].into())),
                ClipboardEntry::String(ClipboardString::new(path.display().to_string())),
            ],
        })
    });
    cx.simulate_keystrokes(&format!("{PRIMARY}-v"));
    assert_eq!(source(&editor, &mut cx), "![](</My Notes/shot.png>)");
}

/// And a file that is not a picture still pastes as its path, which is what
/// the text beside it was for.
#[gpui::test]
fn a_copied_file_that_is_not_a_picture_pastes_its_path(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("", cx);
    let path = std::path::PathBuf::from("/tmp/notes.txt");
    cx.update(|_, cx| {
        cx.write_to_clipboard(ClipboardItem {
            entries: vec![
                ClipboardEntry::ExternalPaths(ExternalPaths(vec![path.clone()].into())),
                ClipboardEntry::String(ClipboardString::new(path.display().to_string())),
            ],
        })
    });
    cx.simulate_keystrokes(&format!("{PRIMARY}-v"));
    assert_eq!(source(&editor, &mut cx), "/tmp/notes.txt");
}

/// Which editor the store was last asked on behalf of. A `fn` carries nothing,
/// which is the whole point — everything it needs comes in as an argument, and
/// a test is the one place with nowhere else to put the answer.
static ASKED: Mutex<Option<EntityId>> = Mutex::new(None);

fn keep(
    source: Source,
    editor: &Entity<Editor>,
    base: Option<&std::path::Path>,
    _: &App,
) -> Option<String> {
    *ASKED.lock().unwrap() = Some(editor.entity_id());
    if let Some(base) = base {
        return Some(format!("{}/{}", base.display(), path_name(&source)?));
    }
    match source {
        Source::File(path) => Some(format!("media://{}", path.file_name()?.to_str()?)),
        Source::Bytes(_) => None,
    }
}

fn path_name(source: &Source) -> Option<String> {
    match source {
        Source::File(path) => Some(path.file_name()?.to_str()?.to_string()),
        Source::Bytes(_) => None,
    }
}

/// Put `path` on the clipboard the way a file manager does — the file itself,
/// and its path as text beside it.
fn copy_file(path: &str, cx: &mut VisualTestContext) {
    let path = std::path::PathBuf::from(path);
    cx.update(|_, cx| {
        cx.write_to_clipboard(ClipboardItem {
            entries: vec![
                ClipboardEntry::ExternalPaths(ExternalPaths(vec![path.clone()].into())),
                ClipboardEntry::String(ClipboardString::new(path.display().to_string())),
            ],
        })
    });
}

/// The store is told which document is asking, so an app holding two of them
/// answers for the right one. The bare `fn` it replaced could only be told by
/// a global the app had to keep in step by hand.
#[gpui::test]
fn the_store_is_told_which_editor_is_asking(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("", cx);
    cx.update(|_, cx| {
        editor::set_image_store(
            cx,
            ImageStore {
                keep,
                ..ImageStore::default()
            },
        )
    });
    copy_file("/My Notes/shot.png", &mut cx);
    cx.simulate_keystrokes(&format!("{PRIMARY}-v"));

    assert_eq!(source(&editor, &mut cx), "![](media://shot.png)");
    assert_eq!(
        *ASKED.lock().unwrap(),
        Some(editor.entity_id()),
        "the store was asked on behalf of the editor that pasted"
    );
}

/// What counts as a picture is the app's to widen. The default guesses from
/// the extension, and an app with its own decoder says so.
#[gpui::test]
fn a_store_decides_for_itself_what_a_picture_is(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("", cx);
    cx.update(|_, cx| {
        editor::set_image_store(
            cx,
            ImageStore {
                keep,
                accepts: |path| path.extension().is_some_and(|ext| ext == "heic"),
            },
        )
    });
    copy_file("/My Notes/shot.heic", &mut cx);
    cx.simulate_keystrokes(&format!("{PRIMARY}-v"));
    assert_eq!(source(&editor, &mut cx), "![](media://shot.heic)");
}

/// And it narrows as well as widens: a `.png` the store does not claim stays
/// the path it was, even though the default guess would have taken it.
#[gpui::test]
fn a_file_the_store_refuses_pastes_as_its_path(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("", cx);
    cx.update(|_, cx| {
        editor::set_image_store(
            cx,
            ImageStore {
                keep,
                accepts: |path| path.extension().is_some_and(|ext| ext == "heic"),
            },
        )
    });
    copy_file("/My Notes/shot.png", &mut cx);
    cx.simulate_keystrokes(&format!("{PRIMARY}-v"));
    assert_eq!(source(&editor, &mut cx), "/My Notes/shot.png");
}

/// The caret's trip into the source and back, driven the way the app's own
/// toggle drives it.
#[gpui::test]
fn the_source_keeps_the_caret_and_gives_it_back(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open(cx);
    go_to_block(&editor, &mut cx, 2);
    cx.simulate_keystrokes("right right right");
    let before = head(&editor, &mut cx);
    let document = source(&editor, &mut cx);

    cx.update(|_, cx| editor.update(cx, |editor, cx| editor.toggle_source(cx)));
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| editor.read(cx).mode()),
        editor::Mode::Source
    );
    assert_eq!(
        source(&editor, &mut cx),
        document,
        "the source view holds exactly what a save would write"
    );
    let at = head(&editor, &mut cx);
    assert_eq!(at.part, markdown::Part::Code, "one text, the fence's");
    assert!(at.offset > 0, "and the caret came with it");

    cx.update(|_, cx| editor.update(cx, |editor, cx| editor.toggle_source(cx)));
    cx.run_until_parked();
    assert_eq!(head(&editor, &mut cx), before, "and goes back where it was");
    assert_eq!(source(&editor, &mut cx), document, "with nothing moved");
}

#[gpui::test]
fn typing_in_the_source_is_typing_in_the_document(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("# Title\n\nbody", cx);
    cx.update(|_, cx| editor.update(cx, |editor, cx| editor.toggle_source(cx)));
    cx.run_until_parked();
    // The caret lands where the *text* starts, past the heading's marker, so
    // this walks back onto the markup itself before typing into it.
    cx.simulate_keystrokes("home");
    cx.simulate_input("#");
    cx.update(|_, cx| editor.update(cx, |editor, cx| editor.toggle_source(cx)));
    cx.run_until_parked();

    assert_eq!(
        cx.update(|_, cx| editor.read(cx).doc().blocks[0].kind.clone()),
        markdown::BlockKind::Heading {
            level: 2,
            text: markdown::Text::plain("Title"),
        },
        "a `#` typed into the markup is a heading level when the document comes back"
    );
}

#[gpui::test]
fn enter_in_the_source_is_a_newline_and_undo_crosses_the_switch(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("# Title", cx);
    cx.update(|_, cx| editor.update(cx, |editor, cx| editor.toggle_source(cx)));
    cx.run_until_parked();
    cx.simulate_keystrokes("end enter");
    cx.simulate_input("body");
    assert_eq!(
        source(&editor, &mut cx),
        "# Title\nbody",
        "enter is a newline in the markup rather than a split"
    );

    cx.simulate_keystrokes(&format!("{PRIMARY}-z {PRIMARY}-z {PRIMARY}-z"));
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| editor.read(cx).mode()),
        editor::Mode::Blocks,
        "stepping back over the switch comes back to the document"
    );
    assert_eq!(source(&editor, &mut cx), "# Title");
}

#[gpui::test]
fn the_block_chrome_stays_out_of_the_source(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open(cx);
    cx.update(|_, cx| editor.update(cx, |editor, cx| editor.toggle_source(cx)));
    cx.run_until_parked();

    // The gutter handle is the block chrome that follows the caret, so it is
    // the one that would show up on a fence holding a whole document.
    assert!(
        !cx.debug_bounds(editor::BLOCK_HANDLE).is_some(),
        "no handle: there are no blocks to drag"
    );
    // Turning "the block" into a heading would wrap the markup in one.
    cx.update(|_, cx| {
        editor.update(cx, |editor, cx| {
            editor.set_block(
                0,
                markdown::BlockKind::Heading {
                    level: 1,
                    text: markdown::Text::default(),
                },
                cx,
            );
        });
    });
    assert_eq!(
        cx.update(|_, cx| editor.read(cx).mode()),
        editor::Mode::Source
    );
    assert!(
        source(&editor, &mut cx).starts_with("# Title"),
        "and the source is untouched"
    );
}

/// Emptying the source is the one edit that can take the fence holding it
/// away, which would leave the caret in a block the source view never paints.
#[gpui::test]
fn the_source_survives_being_emptied(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("# Title\n\nbody", cx);
    cx.update(|_, cx| editor.update(cx, |editor, cx| editor.toggle_source(cx)));
    cx.run_until_parked();

    cx.simulate_keystrokes(&format!("{PRIMARY}-a backspace backspace backspace"));
    cx.run_until_parked();
    assert_eq!(source(&editor, &mut cx), "", "the source is empty");
    assert_eq!(
        head(&editor, &mut cx).part,
        markdown::Part::Code,
        "and the caret is still in the text the source view paints"
    );

    cx.simulate_input("hi");
    cx.update(|_, cx| editor.update(cx, |editor, cx| editor.toggle_source(cx)));
    cx.run_until_parked();
    assert_eq!(
        source(&editor, &mut cx),
        "hi",
        "and typing carries back out"
    );
}

/// The one read a toolbar takes per frame, over the states it has to tell
/// apart.
#[gpui::test]
fn formatting_answers_for_the_whole_bar(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("# Title\n\n**bold** tail", cx);
    let formatting = |cx: &mut VisualTestContext| cx.update(|_, cx| editor.read(cx).formatting());

    let at_title = formatting(&mut cx);
    assert_eq!(at_title.block.as_deref(), Some("Heading 1"));
    assert!(at_title.marks.is_empty(), "nothing marked at the start");
    assert!(!at_title.fenceable, "and one line is not a fence");

    // Into the paragraph, and through the bold run: the caret picks the mark up
    // at the end of the run, which is where a typed character would join it.
    go_to_block(&editor, &mut cx, 1);
    cx.simulate_keystrokes("right right right right");
    let in_bold = formatting(&mut cx);
    assert_eq!(in_bold.block.as_deref(), Some("Text"));
    assert_eq!(in_bold.marks, vec![markdown::Mark::Bold]);

    // cmd-B at a collapsed caret outside the run is a stored mark, and the
    // button that took it has to stay lit until something spends it.
    cx.simulate_keystrokes(&format!("end {PRIMARY}-b"));
    assert_eq!(formatting(&mut cx).marks, vec![markdown::Mark::Bold]);

    // And in the source there is nothing to light.
    cx.update(|_, cx| editor.update(cx, |editor, cx| editor.toggle_source(cx)));
    cx.run_until_parked();
    let in_source = formatting(&mut cx);
    assert_eq!(in_source.mode, editor::Mode::Source);
    assert!(in_source.marks.is_empty() && !in_source.fenceable);
}

#[gpui::test]
fn a_selection_over_two_blocks_reads_as_fenceable(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("one\n\ntwo", cx);
    cx.simulate_keystrokes("shift-down shift-end");
    let formatting = cx.update(|_, cx| editor.read(cx).formatting());
    assert!(
        formatting.fenceable,
        "cmd-E over two blocks makes a fence, which only the editor can say"
    );
}

/// An editor built with something turned off — the app's own chrome in the same
/// place, or a document meant to carry none.
fn open_built(
    source: &str,
    build: impl FnOnce(Editor) -> Editor,
    cx: &mut TestAppContext,
) -> (Entity<Editor>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
    });
    let window = cx.add_window(|_, cx| build(Editor::new(source, cx)));
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

#[gpui::test]
fn chrome_turned_off_never_reaches_the_screen(cx: &mut TestAppContext) {
    let plain = editor::Chrome {
        handle: false,
        slash: false,
        ..Default::default()
    };
    let (editor, mut cx) = open_built("# Title", move |editor| editor.with_chrome(plain), cx);

    assert!(
        cx.debug_bounds(editor::BLOCK_HANDLE).is_none(),
        "no gutter handle on a document that asked for none"
    );
    cx.simulate_keystrokes("end enter");
    cx.simulate_input("/");
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(editor::SLASH_MENU).is_none(),
        "and the slash is a slash"
    );
    assert_eq!(
        source(&editor, &mut cx),
        "# Title\n\n/",
        "which is typed into the document like any other character"
    );
}

#[gpui::test]
fn an_editor_can_open_on_its_source(cx: &mut TestAppContext) {
    let (editor, mut cx) = open_built(
        "# Title\n\nbody",
        |editor| editor.with_mode(editor::Mode::Source),
        cx,
    );

    assert_eq!(
        cx.update(|_, cx| editor.read(cx).mode()),
        editor::Mode::Source
    );
    assert_eq!(source(&editor, &mut cx), "# Title\n\nbody");
    assert_eq!(
        head(&editor, &mut cx).part,
        markdown::Part::Code,
        "the caret is in the text the source view paints"
    );
}

#[gpui::test]
fn an_app_mark_survives_the_editor(cx: &mut TestAppContext) {
    let marks = markdown::Marks::new().with("highlight", "==");
    let (editor, mut cx) = open_built("a ==lit== word", move |editor| editor.with_marks(marks), cx);

    let highlight = markdown::Mark::Custom("highlight".into());
    cx.simulate_keystrokes("right right right");
    assert_eq!(
        cx.update(|_, cx| editor.read(cx).formatting().marks),
        vec![highlight.clone()],
        "a mark the library has never heard of lights a button like any other"
    );
    assert_eq!(
        source(&editor, &mut cx),
        "a ==lit== word",
        "and is written back with the delimiter that spells it"
    );

    cx.update(|_, cx| {
        editor.update(cx, |editor, cx| {
            editor.select(
                markdown::Selection::new(
                    markdown::Cursor::new(0, markdown::Part::Body, 2),
                    markdown::Cursor::new(0, markdown::Part::Body, 5),
                ),
                cx,
            );
            editor.toggle_mark(highlight, cx);
        })
    });
    assert_eq!(
        source(&editor, &mut cx),
        "a lit word",
        "and the same toggle takes it off again"
    );
}

#[gpui::test]
fn platform_ranges_use_utf16_for_chinese_and_emoji(cx: &mut TestAppContext) {
    use gpui::EntityInputHandler;
    let (_, window, _) = open_with("中😀abc!", cx);
    window
        .update(cx, |editor, window, cx| {
            editor.select(
                markdown::Selection::at(markdown::Cursor {
                    offset: 7,
                    ..Default::default()
                }),
                cx,
            );
            assert_eq!(
                editor.selected_text_range(false, window, cx).unwrap().range,
                3..3
            );
            let mut adjusted = None;
            assert_eq!(
                editor
                    .text_for_range(1..3, &mut adjusted, window, cx)
                    .as_deref(),
                Some("😀")
            );
            assert_eq!(adjusted, Some(1..3));
            editor.replace_text_in_range(Some(1..3), "文", window, cx);
            assert_eq!(editor.source().trim(), "中文abc!");
            editor.replace_and_mark_text_in_range(Some(2..5), "😀文", Some(0..2), window, cx);
            assert_eq!(editor.source().trim(), "中文😀文!");
            assert_eq!(editor.marked_text_range(window, cx), Some(2..5));
            assert_eq!(
                editor.selected_text_range(false, window, cx).unwrap().range,
                2..4
            );
            editor.replace_and_mark_text_in_range(None, "中文", Some(1..1), window, cx);
            assert_eq!(editor.source().trim(), "中文中文!");
            assert_eq!(
                editor.selected_text_range(false, window, cx).unwrap().range,
                3..3
            );
            editor.replace_text_in_range(None, "文", window, cx);
            assert_eq!(editor.source().trim(), "中文文!");
            assert_eq!(editor.marked_text_range(window, cx), None);
        })
        .unwrap();
}

#[gpui::test]
fn a_press_on_a_checkbox_toggles_it_and_leaves_the_caret(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("- [ ] open\n- [x] done", cx);
    let caret = head(&editor, &mut cx);

    let box_ = cx
        .update(|_, cx| editor.read(cx).layouts().checkbox_bounds(0))
        .expect("the first checkbox painted");
    cx.simulate_click(box_.center(), gpui::Modifiers::default());

    assert_eq!(
        source(&editor, &mut cx),
        "- [x] open\n- [x] done",
        "the unchecked box checked"
    );
    assert_eq!(head(&editor, &mut cx), caret, "and the caret stayed put");

    let box_ = cx
        .update(|_, cx| editor.read(cx).layouts().checkbox_bounds(1))
        .expect("the second checkbox painted");
    cx.simulate_click(box_.center(), gpui::Modifiers::default());
    assert_eq!(
        source(&editor, &mut cx),
        "- [x] open\n- [ ] done",
        "and the checked one unchecked"
    );

    cx.simulate_keystrokes(&format!("{PRIMARY}-z"));
    assert_eq!(
        source(&editor, &mut cx),
        "- [x] open\n- [x] done",
        "one toggle is one undo step"
    );
}

#[gpui::test]
fn a_press_beside_a_checkbox_places_a_caret(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("- [ ] open\n- [x] done", cx);

    let box_ = cx
        .update(|_, cx| editor.read(cx).layouts().checkbox_bounds(1))
        .expect("the second checkbox painted");
    // The gutter between the box and the text, which belongs to the row.
    cx.simulate_click(
        gpui::point(box_.origin.x + box_.size.width + px(3.0), box_.center().y),
        gpui::Modifiers::default(),
    );

    assert_eq!(
        source(&editor, &mut cx),
        "- [ ] open\n- [x] done",
        "nothing toggled"
    );
    assert_eq!(head(&editor, &mut cx).block, 1, "the caret moved there");
}

/// The mirror of Enter at the end of a quote: at its start the quote goes down
/// whole, and what opens above it is plain text.
#[gpui::test]
fn enter_at_the_start_of_an_alert_opens_plain_text_above(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("> [!NOTE]\n> body", cx);
    cx.simulate_keystrokes("home enter up");
    cx.simulate_input("above");
    assert_eq!(source(&editor, &mut cx), "above\n\n> [!NOTE]\n> body");
}

/// `> [!TIP]` with nothing under it is a document markdown writes down and
/// reads back, and Enter in one has nothing to push down.
#[gpui::test]
fn enter_in_an_empty_alert_keeps_it(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("> [!TIP]", cx);
    cx.simulate_keystrokes("enter");
    assert_eq!(source(&editor, &mut cx), "> [!TIP]");
}

fn copy_text(text: &str, cx: &mut VisualTestContext) {
    cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.to_string())));
}

#[gpui::test]
fn a_paste_in_source_goes_into_the_text_at_the_caret(cx: &mut TestAppContext) {
    let (editor, mut cx) = open_built(
        "# Title\n\nbody",
        |editor| editor.with_mode(editor::Mode::Source),
        cx,
    );
    let at = head(&editor, &mut cx).offset;
    let pasted = "- a\n- b\n\n";
    copy_text(pasted, &mut cx);
    cx.simulate_keystrokes(&format!("{PRIMARY}-v"));

    let mut expected = String::from("# Title\n\nbody");
    expected.insert_str(at, pasted);
    assert_eq!(source(&editor, &mut cx), expected, "spliced into the text");
    let caret = head(&editor, &mut cx);
    assert_eq!(
        (caret.block, caret.part, caret.offset),
        (0, markdown::Part::Code, at + pasted.len()),
        "and the caret is after what was pasted"
    );
}

#[gpui::test]
fn a_url_pasted_over_a_selection_in_source_replaces_it(cx: &mut TestAppContext) {
    let (editor, mut cx) = open_built("body", |editor| editor.with_mode(editor::Mode::Source), cx);
    cx.simulate_keystrokes(&format!("{PRIMARY}-a"));
    copy_text("https://example.com", &mut cx);
    cx.simulate_keystrokes(&format!("{PRIMARY}-v"));
    assert_eq!(source(&editor, &mut cx), "https://example.com");
}

#[gpui::test]
fn a_copied_image_file_pastes_its_path_in_source(cx: &mut TestAppContext) {
    let (editor, mut cx) = open_built("", |editor| editor.with_mode(editor::Mode::Source), cx);
    copy_file("/tmp/shot.png", &mut cx);
    cx.simulate_keystrokes(&format!("{PRIMARY}-v"));
    assert_eq!(source(&editor, &mut cx), "/tmp/shot.png");
}

fn clipboard(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
}

#[gpui::test]
fn a_copy_in_source_is_the_text_without_a_fence(cx: &mut TestAppContext) {
    let (editor, mut cx) = open_built(
        "# Title\n\nbody",
        |editor| editor.with_mode(editor::Mode::Source),
        cx,
    );
    cx.simulate_keystrokes(&format!("{PRIMARY}-a {PRIMARY}-c"));
    assert_eq!(clipboard(&mut cx).as_deref(), Some("# Title\n\nbody"));

    cx.simulate_keystrokes(&format!("{PRIMARY}-x"));
    assert_eq!(clipboard(&mut cx).as_deref(), Some("# Title\n\nbody"));
    assert_eq!(source(&editor, &mut cx), "");
}

#[gpui::test]
fn a_copy_inside_a_code_block_is_the_code(cx: &mut TestAppContext) {
    let (editor, mut cx) = open_built("```rust\nlet a = 1;\n```", |editor| editor, cx);
    cx.update(|_, cx| {
        editor.update(cx, |editor, cx| {
            editor.select(
                markdown::Selection::new(
                    markdown::Cursor::new(0, markdown::Part::Code, 4),
                    markdown::Cursor::new(0, markdown::Part::Code, 9),
                ),
                cx,
            )
        })
    });
    cx.simulate_keystrokes(&format!("{PRIMARY}-c"));
    assert_eq!(clipboard(&mut cx).as_deref(), Some("a = 1"));
}

/// The store runs while the editor is being updated, so the base comes to it
/// as an argument rather than through a read of the editor.
#[gpui::test]
fn the_store_is_handed_the_editors_base(cx: &mut TestAppContext) {
    let (editor, mut cx) = open_built("", |editor| editor.with_base("/notes/article"), cx);
    cx.update(|_, cx| {
        editor::set_image_store(
            cx,
            ImageStore {
                keep,
                ..ImageStore::default()
            },
        )
    });
    copy_file("/My Notes/shot.png", &mut cx);
    cx.simulate_keystrokes(&format!("{PRIMARY}-v"));
    assert_eq!(source(&editor, &mut cx), "![](/notes/article/shot.png)");
}

#[gpui::test]
fn the_highlight_chord_toggles_a_registered_highlight(cx: &mut TestAppContext) {
    let marks = markdown::Marks::new().with(editor::HIGHLIGHT_MARK, "==");
    let (editor, mut cx) = open_built("a lit word", move |editor| editor.with_marks(marks), cx);
    cx.update(|_, cx| {
        editor.update(cx, |editor, cx| {
            editor.select(
                markdown::Selection::new(
                    markdown::Cursor::new(0, markdown::Part::Body, 2),
                    markdown::Cursor::new(0, markdown::Part::Body, 5),
                ),
                cx,
            )
        })
    });
    cx.simulate_keystrokes(&format!("{PRIMARY}-shift-h"));
    assert_eq!(source(&editor, &mut cx), "a ==lit== word");
    cx.simulate_keystrokes(&format!("{PRIMARY}-shift-h"));
    assert_eq!(source(&editor, &mut cx), "a lit word", "and back off");
}

#[gpui::test]
fn the_highlight_chord_does_nothing_without_the_mark(cx: &mut TestAppContext) {
    let (editor, _window, mut cx) = open_with("a lit word", cx);
    cx.simulate_keystrokes(&format!("{PRIMARY}-a {PRIMARY}-shift-h"));
    assert_eq!(source(&editor, &mut cx), "a lit word");
}
