//! `cmd-+`/`cmd--` over a drawn document: what the chords move, and how far.
//!
//! Xcode's and Zed's split — the app owns a base size, the reader owns an
//! adjustment over it, and a reset clears the adjustment.

use editor::{Editor, TextSize};
use gpui::{
    AppContext as _, Entity, Focusable, Pixels, TestAppContext, VisualTestContext, px, size,
};

#[cfg(target_os = "macos")]
const PRIMARY: &str = "cmd";
#[cfg(not(target_os = "macos"))]
const PRIMARY: &str = "ctrl";

const SOURCE: &str = "# Title\n\nA paragraph.";

fn open(cx: &mut TestAppContext) -> (Entity<Editor>, VisualTestContext) {
    open_at(None, None, cx)
}

fn open_at(
    sizing: Option<TextSize>,
    base: Option<f32>,
    cx: &mut TestAppContext,
) -> (Entity<Editor>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        if let Some(sizing) = sizing {
            editor::set_text_size(cx, sizing);
        }
    });
    let window = cx.add_window(|_, cx| {
        let editor = Editor::new(SOURCE, cx);
        match base {
            Some(points) => editor.with_text_size(points),
            None => editor,
        }
    });
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

/// The line box the title actually painted in — proof the size reached the
/// glyphs rather than only the number it is held in. A collapsed caret has no
/// bounds, so this selects a character to ask about.
fn painted(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Pixels {
    cx.simulate_keystrokes("shift-right");
    cx.run_until_parked();
    cx.update(|_, cx| {
        editor
            .read(cx)
            .selection_bounds()
            .expect("the selected row painted")
            .size
            .height
    })
}

fn adjustment(cx: &mut VisualTestContext) -> f32 {
    cx.update(|_, cx| editor::text_size_adjustment(cx))
}

#[gpui::test]
fn a_press_steps_the_size_by_a_point(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(cx);
    let before = painted(&editor, &mut cx);
    cx.simulate_keystrokes(&format!("{PRIMARY}-="));
    cx.run_until_parked();
    assert_eq!(adjustment(&mut cx), 1.0);
    assert!(
        painted(&editor, &mut cx) > before,
        "and the text painted larger with it"
    );
}

/// The base is the app's and the chords never touch it — a reset has somewhere
/// to come back to precisely because the adjustment is the only thing that moved.
#[gpui::test]
fn the_chords_leave_the_apps_base_alone(cx: &mut TestAppContext) {
    let (editor, mut cx) = open_at(None, Some(16.0), cx);
    let before = painted(&editor, &mut cx);
    cx.simulate_keystrokes(&format!("{PRIMARY}-= {PRIMARY}-="));
    cx.run_until_parked();
    assert_eq!(adjustment(&mut cx), 2.0);
    assert_eq!(
        cx.update(|_, cx| editor.read(cx).text_size()),
        Some(16.0),
        "the settings size is untouched"
    );
    cx.simulate_keystrokes(&format!("{PRIMARY}-0"));
    cx.run_until_parked();
    assert_eq!(adjustment(&mut cx), 0.0);
    assert_eq!(
        painted(&editor, &mut cx),
        before,
        "and a reset is back at the base exactly"
    );
}

/// The adjustment is shared, so every open document moves together — Zed moves
/// every buffer, not the focused one.
#[gpui::test]
fn every_open_document_moves_together(cx: &mut TestAppContext) {
    let (focused, mut cx) = open_at(None, Some(16.0), cx);
    let other = cx.update(|_, cx| cx.new(|cx| Editor::new(SOURCE, cx).with_text_size(11.0)));
    cx.simulate_keystrokes(&format!("{PRIMARY}-="));
    cx.run_until_parked();
    assert_eq!(adjustment(&mut cx), 1.0, "one adjustment, for both of them");
    assert_eq!(
        cx.update(|_, cx| (focused.read(cx).text_size(), other.read(cx).text_size())),
        (Some(16.0), Some(11.0)),
        "each keeps the base its app set, and rides the same step over it"
    );
}

/// The whole reason the base is in points: an app with its own size for prose
/// sets it once, and moving the *interface* size must not multiply into it.
#[gpui::test]
fn a_base_in_points_ignores_the_apps_interface_size(cx: &mut TestAppContext) {
    let (editor, mut cx) = open_at(None, Some(20.0), cx);
    let before = painted(&editor, &mut cx);
    cx.update(|_, cx| theme::set_base_text_size(18.0, cx));
    cx.run_until_parked();
    assert_eq!(
        painted(&editor, &mut cx),
        before,
        "the article stayed at 20pt while the interface grew around it"
    );
}

/// A press at the ceiling banks up nothing, so one press back down is one step
/// down rather than the first of however many were swallowed.
#[gpui::test]
fn the_range_holds_at_both_ends(cx: &mut TestAppContext) {
    let sizing = TextSize {
        min: 12.0,
        max: 16.0,
        step: 1.0,
    };
    let (editor, mut cx) = open_at(Some(sizing), Some(13.0), cx);
    for _ in 0..20 {
        cx.simulate_keystrokes(&format!("{PRIMARY}-="));
    }
    cx.run_until_parked();
    assert_eq!(adjustment(&mut cx), 3.0, "13pt stopped at the 16pt ceiling");
    let ceiling = painted(&editor, &mut cx);
    cx.simulate_keystrokes(&format!("{PRIMARY}--"));
    cx.run_until_parked();
    assert_eq!(adjustment(&mut cx), 2.0);
    assert!(
        painted(&editor, &mut cx) < ceiling,
        "one press down is one step down"
    );
    for _ in 0..20 {
        cx.simulate_keystrokes(&format!("{PRIMARY}--"));
    }
    cx.run_until_parked();
    assert_eq!(adjustment(&mut cx), -1.0, "and it stops at the floor");
}

/// A host that wants the adjustment to outlive the process can read it and put
/// it back, without the library having anywhere to write it.
#[gpui::test]
fn a_host_can_read_the_adjustment_and_put_it_back(cx: &mut TestAppContext) {
    let (editor, mut cx) = open_at(None, Some(16.0), cx);
    cx.simulate_keystrokes(&format!("{PRIMARY}-= {PRIMARY}-="));
    cx.run_until_parked();
    let stored = adjustment(&mut cx);
    let painted_at = painted(&editor, &mut cx);

    cx.update(|_, cx| editor::reset_text_size(cx));
    cx.run_until_parked();
    assert_ne!(painted(&editor, &mut cx), painted_at);

    cx.update(|_, cx| editor::adjust_text_size(cx, stored));
    cx.run_until_parked();
    assert_eq!(painted(&editor, &mut cx), painted_at, "restored");
}
