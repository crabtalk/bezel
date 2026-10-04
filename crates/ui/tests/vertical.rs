//! Up and down through a multi-line field's rows.

use gpui::{
    Context, Entity, Focusable, Render, TestAppContext, VisualTestContext, Window, div, prelude::*,
    px,
};
use theme::{Appearance, Theme};
use ui::{
    AppExt as _,
    input::{self, CaretShape, Shape, TextField},
};

struct Page(Entity<TextField>);

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(300.0)).child(self.0.clone())
    }
}

fn open(
    cx: &mut TestAppContext,
    shape: Shape,
    caret: CaretShape,
    text: &str,
) -> (Entity<TextField>, VisualTestContext) {
    cx.update(|cx| {
        Theme::install(Appearance::Dark, cx);
        input::init(cx);
        cx.set_caret_blink(false);
        cx.set_caret_shape(caret);
    });
    let window = cx.add_window(|_, cx| Page(cx.new(|cx| TextField::new(cx).with_shape(shape))));
    let page = window.root(cx).unwrap();
    let field = cx.update(|cx| page.read(cx).0.clone());
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| {
        window.focus(&field.read(cx).focus_handle(cx), cx);
        field.update(cx, |field, cx| {
            field.set_content(text.to_string(), cx);
            field.select(0..0, cx);
        });
    });
    visual.run_until_parked();
    (field, visual)
}

fn walk(cx: &mut TestAppContext, shape: Shape, caret: CaretShape, key: &str) -> Vec<usize> {
    let text = "ab\n\n\n";
    let (field, mut visual) = open(cx, shape, caret, text);
    let mut seen = Vec::new();
    for _ in 0..4 {
        visual.simulate_keystrokes(key);
        visual.run_until_parked();
        seen.push(visual.update(|_, cx| field.read(cx).cursor()));
    }
    seen
}

#[gpui::test]
fn down_reaches_every_trailing_empty_line(cx: &mut TestAppContext) {
    for shape in [
        Shape::Grow { min: 1, max: 8 },
        Shape::Grow { min: 1, max: 2 },
    ] {
        for caret in [CaretShape::Bar, CaretShape::Block, CaretShape::Underline] {
            let keys: &[&str] = if cfg!(target_os = "macos") {
                &["down", "ctrl-n"]
            } else {
                &["down"]
            };
            for &key in keys {
                assert_eq!(
                    walk(cx, shape, caret, key),
                    vec![3, 4, 5, 5],
                    "{shape:?} {caret:?} {key}"
                );
            }
        }
    }
}
