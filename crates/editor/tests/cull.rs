//! A long document inside a scrolling page builds only the blocks near the
//! part of it the window shows.

use editor::{Editor, Mode};
use gpui::{
    Context, Entity, Focusable, Pixels, Render, ScrollHandle, TestAppContext, VisualTestContext,
    Window, div, point, prelude::*, px, size,
};
use markdown::{Cursor, Part, Selection};

const SECTIONS: usize = 300;
const HEIGHT: f32 = 600.0;

struct Page {
    editor: Entity<Editor>,
    scroll: ScrollHandle,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("page")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(self.editor.clone())
    }
}

fn source() -> String {
    (0..SECTIONS)
        .map(|ix| {
            // Lengths vary, so a guess made from one is not another's height.
            let words = "word ".repeat(1 + ix * 7 % 60);
            format!("## Section {ix}\n\nParagraph {ix}: {words}\n\n")
        })
        .collect()
}

fn open(
    focused: bool,
    cx: &mut TestAppContext,
) -> (Entity<Editor>, ScrollHandle, VisualTestContext) {
    open_in(Mode::Blocks, focused, cx)
}

fn open_in(
    mode: Mode,
    focused: bool,
    cx: &mut TestAppContext,
) -> (Entity<Editor>, ScrollHandle, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
    });
    let scroll = ScrollHandle::new();
    let window = cx.add_window({
        let scroll = scroll.clone();
        move |_, cx| Page {
            editor: cx.new(|cx| {
                Editor::new(&source(), cx)
                    .with_mode(mode)
                    .with_scroll(scroll.clone())
            }),
            scroll,
        }
    });
    let page = window.root(cx).unwrap();
    let editor = cx.update(|cx| page.read(cx).editor.clone());
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(400.0), px(HEIGHT)));
    if focused {
        visual.update(|window, cx| {
            let handle = editor.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
        });
    }
    frames(&mut visual, 3);
    (editor, scroll, visual)
}

fn frames(cx: &mut VisualTestContext, count: usize) {
    for _ in 0..count {
        cx.update(|window, cx| {
            window.refresh();
            window.simulate_next_frame(cx);
        });
        cx.run_until_parked();
    }
}

fn blocks(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| editor.read(cx).doc().blocks.len())
}

fn built(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Vec<usize> {
    cx.update(|_, cx| {
        let editor = editor.read(cx);
        (0..editor.doc().blocks.len())
            .filter(|ix| editor.layouts().block_bounds(*ix).is_some())
            .collect()
    })
}

/// The block across the top edge of the window, and where its top is.
fn at_top(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> (usize, Pixels) {
    cx.update(|_, cx| {
        let layouts = editor.read(cx).layouts();
        (0..editor.read(cx).doc().blocks.len())
            .find_map(|ix| {
                let bounds = layouts.block_bounds(ix)?;
                (bounds.bottom() > px(0.0)).then_some((ix, bounds.top()))
            })
            .unwrap()
    })
}

#[gpui::test]
fn only_blocks_near_the_viewport_are_built(cx: &mut TestAppContext) {
    let (editor, scroll, mut cx) = open(true, cx);
    let blocks = blocks(&editor, &mut cx);
    let height = scroll.max_offset().y;
    assert!(height > px(10_000.0), "the document overflows: {height:?}");

    let top = built(&editor, &mut cx);
    assert!(top.contains(&0), "{top:?}");
    assert!(top.len() < blocks / 10, "{} of {blocks} built", top.len());

    scroll.set_offset(point(px(0.0), -height));
    frames(&mut cx, 3);
    let bottom = built(&editor, &mut cx);
    assert!(bottom.contains(&(blocks - 1)), "{bottom:?}");
    // The caret stays at the start, and its block is built wherever the page is.
    assert!(bottom.contains(&0), "{bottom:?}");
    assert!(
        bottom.len() < blocks / 10,
        "{} of {blocks} built",
        bottom.len()
    );
}

#[gpui::test]
fn a_jump_builds_what_it_lands_on_in_the_same_frame(cx: &mut TestAppContext) {
    let (editor, scroll, mut cx) = open(true, cx);
    let blocks = blocks(&editor, &mut cx);
    scroll.set_offset(point(px(0.0), -scroll.max_offset().y));
    frames(&mut cx, 1);
    let (ix, top) = at_top(&editor, &mut cx);
    assert!(ix > blocks / 2, "block {ix} at the top after the jump");
    // Nothing blank above it but the gap between two blocks.
    assert!(top <= px(12.0), "{top:?}");
}

/// Blocks never measured are placed at a guess. The first frame that builds
/// them corrects it, and the text already showing does not move.
#[gpui::test]
fn text_showing_stays_put_while_guesses_correct(cx: &mut TestAppContext) {
    let (editor, scroll, mut cx) = open(true, cx);
    scroll.set_offset(point(px(0.0), -scroll.max_offset().y));
    frames(&mut cx, 2);
    scroll.set_offset(point(px(0.0), -scroll.max_offset().y * 0.5));
    frames(&mut cx, 1);
    let before = at_top(&editor, &mut cx);
    frames(&mut cx, 3);
    let (ix, top) = at_top(&editor, &mut cx);
    assert_eq!(ix, before.0);
    assert!((top - before.1).abs() <= px(1.0), "{before:?} → {top:?}");
}

#[gpui::test]
fn a_resize_builds_only_what_shows(cx: &mut TestAppContext) {
    let (editor, _, mut cx) = open(true, cx);
    let blocks = blocks(&editor, &mut cx);
    cx.simulate_resize(size(px(300.0), px(HEIGHT)));
    frames(&mut cx, 1);
    let built = built(&editor, &mut cx);
    assert!(
        built.len() < blocks / 10,
        "{} of {blocks} built",
        built.len()
    );
}

/// A host moving the caret of an editor nobody has focused — a jump to a
/// heading from an outline — still gets the block built and revealed.
#[gpui::test]
fn select_reveals_a_block_off_screen(cx: &mut TestAppContext) {
    let (editor, _, mut cx) = open(false, cx);
    let last = blocks(&editor, &mut cx) - 1;
    cx.update(|_, cx| {
        editor.update(cx, |editor, cx| {
            editor.select(Selection::at(Cursor::new(last, Part::Body, 0)), cx)
        })
    });
    frames(&mut cx, 3);
    let bounds = cx
        .update(|_, cx| editor.read(cx).layouts().block_bounds(last))
        .expect("the selected block is built");
    // Revealing scrolls the caret's row into view, not the whole block.
    assert!(
        bounds.top() >= px(0.0) && bounds.top() < px(HEIGHT),
        "{bounds:?}"
    );
}

/// Source mode is one text, built a line at a time.
#[gpui::test]
fn source_mode_builds_only_the_lines_near_the_viewport(cx: &mut TestAppContext) {
    let (editor, scroll, mut cx) = open_in(Mode::Source, true, cx);
    let end = cx.update(|_, cx| editor.read(cx).source().len());
    let painted = |at: usize, cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            editor
                .read(cx)
                .layouts()
                .position(Cursor::new(0, Part::Code, at))
                .is_some()
        })
    };
    assert!(painted(0, &mut cx));
    assert!(
        !painted(end, &mut cx),
        "the last line is not built at the top"
    );

    scroll.set_offset(point(px(0.0), -scroll.max_offset().y));
    frames(&mut cx, 1);
    assert!(
        painted(end, &mut cx),
        "the last line is built at the bottom"
    );
}
