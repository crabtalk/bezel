use gpui::{Context, Render, TestAppContext, VisualTestContext, Window, div, prelude::*, px};
use markdown::{BlockLayouts, Cursor, Editing, Part, Selection};
use theme::{Appearance, Theme};
use ui::{AppExt as _, input::CaretShape};

struct Page {
    mode: usize,
    text: String,
    offset: usize,
    /// Where the selection's anchor sits; `None` collapses it on the caret.
    anchor: Option<usize>,
    layouts: BlockLayouts,
    virtualized: bool,
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let cursor = Cursor::new(
            0,
            if self.mode == 0 {
                Part::Body
            } else {
                Part::Code
            },
            self.offset,
        );
        let editing = Editing {
            selection: Some(Selection::new(
                Cursor::new(
                    cursor.block,
                    cursor.part,
                    self.anchor.unwrap_or(self.offset),
                ),
                cursor,
            )),
            layouts: self.virtualized.then_some(&self.layouts),
            ..Editing::default()
        };
        let body = if self.mode == 2 {
            markdown::render_source(&self.text, editing, cx)
        } else {
            let source = if self.mode == 1 {
                format!("```\n{}\n```", self.text)
            } else {
                self.text.clone()
            };
            let mut doc = markdown::parse(&source);
            if doc.blocks.is_empty() {
                doc.blocks
                    .push(markdown::Block::new(markdown::BlockKind::Paragraph(
                        markdown::Text::default(),
                    )));
            }
            markdown::render_with(&doc, editing, window, cx)
        };
        div().w(px(300.0)).child(body)
    }
}

fn caret(cx: &mut VisualTestContext, shape: CaretShape) -> gpui::Quad {
    cx.update(|_, cx| cx.set_caret_shape(shape));
    cx.run_until_parked();
    cx.update(|window, cx| {
        let color = Theme::of(cx).caret;
        // Solid in the caret's own colour; a block in an inactive window is
        // its outline.
        window
            .painted_quads()
            .into_iter()
            .find(|quad| {
                quad.background == color.into()
                    || (shape == CaretShape::Block && quad.border_color == color)
            })
            .expect("caret is painted")
    })
}

#[gpui::test]
fn document_carets_share_shapes_in_all_render_paths(cx: &mut TestAppContext) {
    for appearance in [Appearance::Dark, Appearance::Light] {
        cx.update(|cx| Theme::install(appearance, cx));
        for mode in 0..3 {
            for virtualized in [false, true] {
                let window = cx.add_window(|_, _| Page {
                    mode,
                    text: "Wi中".into(),
                    offset: 0,
                    anchor: None,
                    layouts: BlockLayouts::default(),
                    virtualized,
                });
                let page = window.root(cx).unwrap();
                let mut visual = VisualTestContext::from_window(window.into(), cx);
                let bar = caret(&mut visual, CaretShape::Bar);
                assert_eq!(
                    bar.bounds.size.width,
                    gpui::ScaledPixels(1.5 * visual.update(|window, _| window.scale_factor()))
                );
                let block = caret(&mut visual, CaretShape::Block);
                let underline = caret(&mut visual, CaretShape::Underline);
                // A block fills the line; the bar is the font's height inside it.
                assert_eq!(block.bounds.origin.x, bar.bounds.origin.x);
                assert!(block.bounds.size.height > bar.bounds.size.height);
                assert!((block.bounds.center().y.0 - bar.bounds.center().y.0).abs() <= 1.0);
                assert!(block.bounds.size.width > bar.bounds.size.width);
                assert_eq!(underline.bounds.size.width, block.bounds.size.width);
                assert_eq!(
                    underline.bounds.size.height,
                    gpui::ScaledPixels(2.0 * visual.update(|window, _| window.scale_factor()))
                );
                assert_eq!(underline.bounds.bottom(), bar.bounds.bottom());
                visual.update(|_, cx| {
                    page.update(cx, |page, cx| {
                        page.offset = 1;
                        cx.notify();
                    })
                });
                let narrow = caret(&mut visual, CaretShape::Block);
                if mode == 0 {
                    assert!(narrow.bounds.size.width < block.bounds.size.width);
                }
                // Where no character follows, a block is as wide as a `0`.
                visual.update(|_, cx| {
                    page.update(cx, |page, cx| {
                        page.text = "0".into();
                        page.offset = 0;
                        cx.notify();
                    })
                });
                let zero = caret(&mut visual, CaretShape::Block).bounds.size.width;
                for empty in [false, true] {
                    visual.update(|_, cx| {
                        page.update(cx, |page, cx| {
                            if empty {
                                page.text.clear();
                            }
                            page.offset = page.text.len();
                            cx.notify();
                        })
                    });
                    let end = caret(&mut visual, CaretShape::Block);
                    // Painted quads snap to device pixels.
                    assert!((end.bounds.size.width.0 - zero.0).abs() <= 1.0);
                }
            }
        }
    }
}

#[gpui::test]
fn no_caret_over_a_selection_in_any_render_path(cx: &mut TestAppContext) {
    cx.update(|cx| Theme::install(Appearance::Dark, cx));
    for mode in 0..3 {
        for shape in [CaretShape::Bar, CaretShape::Block, CaretShape::Underline] {
            let window = cx.add_window(|_, _| Page {
                mode,
                text: "Wide".into(),
                offset: 2,
                anchor: Some(0),
                layouts: BlockLayouts::default(),
                virtualized: false,
            });
            let mut visual = VisualTestContext::from_window(window.into(), cx);
            visual.update(|_, cx| cx.set_caret_shape(shape));
            visual.run_until_parked();
            let painted = visual.update(|window, cx| {
                let color = Theme::of(cx).caret;
                window
                    .painted_quads()
                    .into_iter()
                    .any(|quad| quad.background == color.into() || quad.border_color == color)
            });
            assert!(
                !painted,
                "caret painted over a selection: mode {mode}, {shape:?}"
            );
        }
    }
}
