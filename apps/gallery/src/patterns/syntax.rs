//! The syntax screen — every grammar `syntax` highlights.
//!
//! Nothing here is library code. The page picks a sample and hands it to
//! [`markdown::render`] as a one-fence document. Copy this file.
//!
//! Samples are chosen so each reaches token kinds the others do not — a wrong
//! palette color has to show up on some screen, and this is it.

use gpui::{Context, ElementId, Render, ScrollHandle, Window, div, prelude::*, px};
use markdown::Doc;
use theme::Theme;
use ui::{
    scroll::{self, TransientState},
    widgets::Controls,
};

use crate::{hint, patterns::samples::SAMPLES, stack};

pub struct Syntax {
    /// One parsed document per sample, built once — parsing on every render
    /// would put a markdown parse in the scroll path for no gain.
    docs: Vec<Doc>,
    selected: usize,
    scroll: ScrollHandle,
    bar: TransientState,
}

impl Syntax {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            docs: SAMPLES
                .iter()
                .map(|(tag, _, code)| markdown::parse(&format!("```{tag}\n{code}\n```")))
                .collect(),
            selected: 0,
            scroll: ScrollHandle::new(),
            bar: TransientState::new(),
        }
    }
}

impl Render for Syntax {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let doc = self.docs[self.selected].clone();

        div()
            .relative()
            .size_full()
            .child(
                div()
                    .id("syntax-page")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(
                        stack()
                            .child(hint(
                                &theme,
                                "Tree-sitter highlighting for fenced code blocks. A fence tag \
                 names the grammar, `syntax::highlight` returns spans, and \
                 `markdown::render` recolors runs without moving layout — the \
                 block is laid out line by line, so highlighting never shifts \
                 it.",
                            ))
                            .child(
                                theme
                                    .toggle_group()
                                    .children(SAMPLES.iter().enumerate().map(
                                        |(index, (_, label, _))| {
                                            theme
                                                .toggle_group_item(*label, self.selected == index)
                                                .id(ElementId::Name((*label).into()))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.selected = index;
                                                    cx.notify();
                                                }))
                                        },
                                    )),
                            )
                            .child(div().max_w(px(680.0)).child(markdown::render(
                                &doc,
                                markdown::Caption::Shown,
                                window,
                                cx,
                            ))),
                    ),
            )
            .child(scroll::transient(
                "syntax-bar",
                &self.scroll,
                &self.bar,
                cx.reduce_motion(),
            ))
    }
}
