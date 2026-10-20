//! The Mermaid playground — source on the left, its diagram on the right,
//! drawn again as you type.
//!
//! The preview is a one-fence document handed to [`markdown::render`], parsed
//! again on every edit. Copy this file.

use gpui::{Context, Entity, Render, ScrollHandle, Subscription, Window, div, prelude::*, px};
use markdown::Doc;
use motion::Painter;
use theme::Theme;
use ui::{
    combobox::{Combobox, ComboboxEvent},
    input::{FieldEvent, Shape, TextField},
    scroll::{self, Axes, TransientState},
};

use super::mermaid::SAMPLES;
use crate::{hint, section_at, stack};

pub struct Playground {
    source: Entity<TextField>,
    samples: Entity<Combobox>,
    preview: Doc,
    scroll: ScrollHandle,
    bar: TransientState,
    _subscriptions: [Subscription; 2],
}

fn fenced(source: &str) -> Doc {
    markdown::parse(&format!("```mermaid\n{source}\n```"))
}

impl Playground {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let first = SAMPLES[0].source;
        let source = cx.new(|cx| {
            let mut field = TextField::new(cx).with_shape(Shape::Fill);
            field.set_content(first, cx);
            field
        });
        let samples = cx.new(|cx| {
            Combobox::new(
                SAMPLES
                    .iter()
                    .map(|sample| {
                        section_at(sample.key)
                            .map_or(sample.key, |section| section.title)
                            .into()
                    })
                    .collect(),
                "Load an example",
                cx,
            )
            .with_selection(0)
        });
        let edited = cx.subscribe(&source, |this, source, event, cx| {
            if let FieldEvent::Changed(_) = event {
                this.preview = fenced(source.read(cx).content());
                cx.notify();
            }
        });
        let picked = cx.subscribe(&samples, |this, _, event, cx| {
            let ComboboxEvent::Selected(index) = event;
            let source = SAMPLES[*index].source;
            this.source
                .update(cx, |field, cx| field.set_content(source, cx));
            this.preview = fenced(source);
            cx.notify();
        });
        Self {
            source,
            samples,
            preview: fenced(first),
            scroll: ScrollHandle::new(),
            bar: TransientState::new(Painter::of(cx)),
            _subscriptions: [edited, picked],
        }
    }
}

impl Render for Playground {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let editor = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .font_family(theme.font_mono.clone())
            .child(self.source.clone());
        let preview = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                scroll::pane("playground-preview", Axes::Vertical)
                    .size_full()
                    .track_scroll(&self.scroll)
                    .child(markdown::render(
                        &self.preview,
                        markdown::Caption::Shown,
                        window,
                        cx,
                    )),
            )
            .child(scroll::transient(
                "playground-bar",
                &self.scroll,
                &self.bar,
                cx.reduce_motion(),
            ));
        stack()
            .size_full()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.0))
                    .child(hint(
                        &theme,
                        "Mermaid source, drawn as you type. Source that does not draw shows \
                         the error under it.",
                    ))
                    .child(div().w(px(220.0)).flex_none().child(self.samples.clone())),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .gap(px(24.0))
                    .child(editor)
                    .child(preview),
            )
    }
}
