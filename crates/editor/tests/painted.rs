//! Fences an app paints: the slash rows that make them, the rewrite they are
//! lent, and the keys typed inside one.

use std::cell::RefCell;

use editor::{AppExt as _, Editor};
use gpui::{
    AnyElement, App, Entity, FocusHandle, Focusable, TestAppContext, VisualTestContext, Window,
    div, prelude::*, px, size,
};
use markdown::{AppExt as _, BlockKind, Fence, Text};

thread_local! {
    /// The focus the painted block tracks, made by the first paint.
    static INSIDE: RefCell<Option<FocusHandle>> = const { RefCell::new(None) };
}

fn paint(fence: &Fence<'_>, _: &mut Window, cx: &mut App) -> Option<AnyElement> {
    if fence.language != "widget" {
        return None;
    }
    let handle = INSIDE.with(|inside| {
        inside
            .borrow_mut()
            .get_or_insert_with(|| cx.focus_handle())
            .clone()
    });
    Some(
        div()
            .id("widget")
            .h(px(40.0))
            .track_focus(&handle)
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .into_any_element(),
    )
}

fn open(source: &str, cx: &mut TestAppContext) -> (Entity<Editor>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        cx.set_block_renderer(paint);
        cx.set_slash_items(vec![editor::SlashItem::Group {
            label: "Widget".into(),
            rows: vec![(
                "Blue".into(),
                editor::SlashAction::Run(std::rc::Rc::new(|at, _, cx| {
                    let kind = BlockKind::Code {
                        language: Some("widget".to_owned()),
                        code: Text::plain("blue"),
                    };
                    at.editor
                        .update(cx, |editor, cx| editor.place_block(at.block, kind, cx))
                        .ok();
                })),
            )],
        }]);
    });
    let window = cx.add_window(|_, cx| Editor::new(source, cx));
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

fn source(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| editor.read(cx).source())
}

#[gpui::test]
fn an_app_row_makes_its_block_and_leaves_the_caret_after_it(cx: &mut TestAppContext) {
    let (editor, mut cx) = open("# Title", cx);
    cx.simulate_keystrokes("end enter");
    cx.simulate_input("/blue");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    cx.simulate_input("after");
    assert_eq!(
        source(&editor, &mut cx),
        "# Title\n\n```widget\nblue\n```\n\nafter"
    );
}

#[gpui::test]
fn set_code_rewrites_the_fence(cx: &mut TestAppContext) {
    let (editor, mut cx) = open("```widget\n```\n", cx);
    cx.update(|_, cx| editor.update(cx, |editor, cx| editor.set_code(0, "link".to_owned(), cx)));
    assert_eq!(source(&editor, &mut cx), "```widget\nlink\n```");
}

#[gpui::test]
fn keys_inside_a_painted_fence_stay_out_of_the_document(cx: &mut TestAppContext) {
    let (editor, mut cx) = open("text\n\n```widget\n```\n", cx);
    let inside = INSIDE
        .with(|inside| inside.borrow().clone())
        .expect("painted");
    let before = source(&editor, &mut cx);
    cx.update(|window, cx| window.focus(&inside, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("backspace enter cmd-a");
    assert_eq!(source(&editor, &mut cx), before);
    cx.simulate_keystrokes("escape");
    let focused = cx.update(|window, cx| editor.read(cx).focus_handle(cx).is_focused(window));
    assert!(focused, "escape hands the keyboard back to the document");
}
