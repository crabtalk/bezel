//! The query, filter and result rows shared by searchable pickers.

use gpui::{Context, Entity, SharedString, Window, div, prelude::*, px};
use theme::{TextStyle, Theme, Typeset};

use crate::{
    icons,
    input::{FieldEvent, TextField},
    popover,
};

pub(crate) struct SearchList {
    pub query: Entity<TextField>,
    pub filter: popover::Filter,
    text: SharedString,
}

impl SearchList {
    pub fn new<V: 'static>(
        items: Vec<SharedString>,
        placeholder: &'static str,
        get: fn(&mut V) -> &mut Self,
        cx: &mut Context<V>,
    ) -> Self {
        let query = cx.new(|cx| {
            TextField::new(cx)
                .with_placeholder(placeholder)
                .with_frame(false)
        });
        cx.subscribe(&query, move |view, query, event: &FieldEvent, cx| {
            if matches!(event, FieldEvent::Changed) {
                let search = get(view);
                let text = query.read(cx).content();
                if search.text != *text {
                    search.filter.refilter(text);
                    search.text = text.clone();
                    cx.notify();
                }
            }
        })
        .detach();
        Self {
            query,
            filter: popover::Filter::new(items),
            text: "".into(),
        }
    }

    pub fn clear<V: 'static>(&mut self, cx: &mut Context<V>) {
        self.query.update(cx, |query, cx| query.clear(cx));
        self.text = "".into();
        self.filter.refilter("");
    }

    pub fn body<V: 'static>(
        &self,
        theme: &Theme,
        selected: Option<usize>,
        get: fn(&mut V) -> &mut Self,
        choose: fn(&mut V, usize, &mut Window, &mut Context<V>),
        cx: &mut Context<V>,
    ) -> gpui::Div {
        let rows = self
            .filter
            .filtered()
            .iter()
            .enumerate()
            .map(|(position, &item)| {
                popover::menu_row(theme, Some(position) == self.filter.active(), None)
                    .justify_between()
                    .id(("search-result", item))
                    .on_mouse_move(cx.listener(move |view, _, _, cx| {
                        let filter = &mut get(view).filter;
                        if filter.active() != Some(position) {
                            filter.set_active(position);
                            cx.notify();
                        }
                    }))
                    .on_click(
                        cx.listener(move |view, _, window, cx| choose(view, item, window, cx)),
                    )
                    .child(self.filter.items()[item].clone())
                    .when(selected == Some(item), |row| {
                        row.child(
                            icons::icon(icons::glyph::Check)
                                .size(px(13.0))
                                .text_color(theme.text),
                        )
                    })
            });
        div()
            .flex()
            .flex_col()
            .child(popover::search_line(
                theme,
                self.query.clone().into_any_element(),
            ))
            .child(if self.filter.filtered().is_empty() {
                div()
                    .px(px(10.0))
                    .py(px(8.0))
                    .text_style(TextStyle::Body)
                    .text_color(theme.text_muted)
                    .child("No matches")
            } else {
                div().flex().flex_col().children(rows)
            })
    }
}
