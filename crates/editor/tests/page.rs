//! An editor framed by a host page: padding around it, and presses on the
//! padding routed through `Editor::press`.

use editor::Editor;
use gpui::{
    Context, Entity, Focusable, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, Render, TestAppContext, VisualTestContext, Window, div, point,
    prelude::*, px, size,
};

const PADDING: f32 = 60.0;

struct Page {
    editor: Entity<Editor>,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let editor = self.editor.clone();
        div()
            .size_full()
            .p(px(PADDING))
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                editor.update(cx, |editor, cx| {
                    editor.press(
                        event.position,
                        event.click_count,
                        event.modifiers,
                        window,
                        cx,
                    )
                });
            })
            .child(self.editor.clone())
    }
}

fn open(source: &str, cx: &mut TestAppContext) -> (Entity<Editor>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
    });
    let window = cx.add_window(|_, cx| Page {
        editor: cx.new(|cx| Editor::new(source, cx)),
    });
    let page = window.root(cx).unwrap();
    let editor = cx.update(|cx| page.read(cx).editor.clone());
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(400.0), px(600.0)));
    visual.update(|window, cx| {
        let handle = editor.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    visual.run_until_parked();
    (editor, visual)
}

fn down(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_event(MouseDownEvent {
        position: at,
        button: MouseButton::Left,
        modifiers: Modifiers::default(),
        click_count: 1,
        first_mouse: false,
    });
}

fn drag(cx: &mut VisualTestContext, to: Point<Pixels>) {
    cx.simulate_event(MouseMoveEvent {
        position: to,
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::default(),
    });
}

fn up(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_event(MouseUpEvent {
        position: at,
        button: MouseButton::Left,
        modifiers: Modifiers::default(),
        click_count: 1,
    });
}

/// A point inside the first painted line of block `ix`.
fn in_block(editor: &Entity<Editor>, cx: &mut VisualTestContext, ix: usize) -> Point<Pixels> {
    let bounds = cx
        .update(|_, cx| editor.read(cx).layouts().block_bounds(ix))
        .expect("the block painted");
    point(bounds.origin.x + px(4.0), bounds.origin.y + px(4.0))
}

#[gpui::test]
fn a_selection_keeps_extending_past_the_editors_edge(cx: &mut TestAppContext) {
    let (editor, mut cx) = open("first\n\nsecond\n\nthird", cx);
    let start = in_block(&editor, &mut cx, 0);

    down(&mut cx, start);
    // Below the last block and inside the page's padding: outside the editor.
    let outside = point(px(PADDING / 2.0), px(580.0));
    drag(&mut cx, outside);
    up(&mut cx, outside);

    let selection = cx.update(|_, cx| editor.read(cx).selection());
    assert_eq!(selection.head.block, 2, "the drag reached the last block");
}

#[gpui::test]
fn a_press_on_the_page_places_a_caret_and_drags(cx: &mut TestAppContext) {
    let (editor, mut cx) = open("first\n\nsecond\n\nthird", cx);
    let target = in_block(&editor, &mut cx, 1);

    // In the left padding, level with the second block.
    let beside = point(px(PADDING / 2.0), target.y);
    down(&mut cx, beside);
    let selection = cx.update(|_, cx| editor.read(cx).selection());
    assert!(selection.is_collapsed());
    assert_eq!(
        selection.head.block, 1,
        "the caret went to the row beside it"
    );

    let end = in_block(&editor, &mut cx, 2);
    drag(&mut cx, end);
    up(&mut cx, end);
    let selection = cx.update(|_, cx| editor.read(cx).selection());
    assert_eq!(
        (selection.anchor.block, selection.head.block),
        (1, 2),
        "and a drag from there selects"
    );
}

#[gpui::test]
fn a_press_on_the_editor_is_not_taken_twice(cx: &mut TestAppContext) {
    let (editor, mut cx) = open("- [ ] open", cx);
    let box_ = cx
        .update(|_, cx| editor.read(cx).layouts().checkbox_bounds(0))
        .expect("the checkbox painted");

    cx.simulate_click(box_.center(), Modifiers::default());

    let source = cx.update(|_, cx| editor.read(cx).source());
    assert_eq!(source, "- [x] open", "one toggle, not two");
}
