//! A long document inside a scrolling page builds only the blocks near the
//! viewport, and keeps its height while the rest stand in.

use editor::Editor;
use gpui::{
    Context, Entity, Focusable, Render, ScrollHandle, TestAppContext, VisualTestContext, Window,
    div, point, prelude::*, px, size,
};

const SECTIONS: usize = 300;

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
            format!(
                "## Section {ix}\n\nA paragraph long enough to wrap onto a second row at \
                 this width, so a block is taller than one line.\n\n"
            )
        })
        .collect()
}

fn open(cx: &mut TestAppContext) -> (Entity<Editor>, ScrollHandle, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
    });
    let scroll = ScrollHandle::new();
    let window = cx.add_window({
        let scroll = scroll.clone();
        move |_, cx| Page {
            editor: cx.new(|cx| Editor::new(&source(), cx).with_scroll(scroll.clone())),
            scroll,
        }
    });
    let page = window.root(cx).unwrap();
    let editor = cx.update(|cx| page.read(cx).editor.clone());
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(400.0), px(600.0)));
    visual.update(|window, cx| {
        let handle = editor.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
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

fn built(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Vec<usize> {
    cx.update(|_, cx| {
        let editor = editor.read(cx);
        (0..editor.doc().blocks.len())
            .filter(|ix| editor.layouts().block_bounds(*ix).is_some())
            .collect()
    })
}

#[gpui::test]
fn only_blocks_near_the_viewport_are_built(cx: &mut TestAppContext) {
    let (editor, scroll, mut cx) = open(cx);
    let blocks = cx.update(|_, cx| editor.read(cx).doc().blocks.len());
    let height = scroll.max_offset().y;
    assert!(height > px(10_000.0), "the document overflows: {height:?}");

    let top = built(&editor, &mut cx);
    assert!(top.contains(&0), "{top:?}");
    assert!(top.len() < blocks / 4, "{} of {blocks} built", top.len());

    scroll.set_offset(point(px(0.0), -height));
    frames(&mut cx, 3);
    let bottom = built(&editor, &mut cx);
    assert!(bottom.contains(&(blocks - 1)), "{bottom:?}");
    // The caret stays at the start, and its block is built wherever the page is.
    assert!(bottom.contains(&0), "{bottom:?}");
    assert!(
        bottom.len() < blocks / 4,
        "{} of {blocks} built",
        bottom.len()
    );
    assert_eq!(scroll.max_offset().y, height, "stand-ins keep the height");
}
