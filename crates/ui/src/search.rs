//! The query, filter and result rows shared by searchable pickers.

use std::cell::Cell;

use gpui::{Axis, Context, Entity, Point, ScrollHandle, SharedString, Window, div, prelude::*, px};
use theme::{TextStyle, Theme, Typeset};

use crate::{
    icons,
    input::{FieldEvent, TextField},
    popover, scroll,
};

/// How many results a list shows before it scrolls. A count rather than a
/// height: the rows are what the reader is counting, and a figure in pixels
/// would have to be restated every time their metrics move.
const MAX_ROWS: f32 = 12.0;

pub(crate) struct SearchList {
    pub query: Entity<TextField>,
    pub filter: popover::Filter,
    text: SharedString,
    /// The result list's own scroll, so the keyboard can keep the active row
    /// in view: past [`MAX_ROWS`] the arrows reach rows the reader cannot see.
    scroll: ScrollHandle,
    /// The row the list was last scrolled to. A wheel belongs to the reader —
    /// only the active row moving is this list's business to follow.
    scrolled: Cell<Option<usize>>,
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
            if matches!(event, FieldEvent::Changed(_)) {
                let search = get(view);
                let text = query.read(cx).content();
                if search.text != *text {
                    search.filter.refilter(text);
                    search.text = text.clone();
                    search.rewind();
                    cx.notify();
                }
            }
        })
        .detach();
        Self {
            query,
            filter: popover::Filter::new(items),
            text: "".into(),
            scroll: ScrollHandle::new(),
            scrolled: Cell::new(None),
        }
    }

    pub fn clear<V: 'static>(&mut self, cx: &mut Context<V>) {
        self.query.update(cx, |query, cx| query.clear(cx));
        self.text = "".into();
        self.filter.refilter("");
        self.rewind();
    }

    /// Back to the top, for a list that is about to hold different rows —
    /// where the offset it was left at names nothing.
    fn rewind(&self) {
        self.scroll.set_offset(Point::default());
        self.scrolled.set(None);
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
        // Following the active row, not leading it: the arrows move it without
        // knowing what is on screen, and the mouse sets it to a row that is on
        // screen by definition.
        if self.scrolled.get() != self.filter.active() {
            self.scrolled.set(self.filter.active());
            if let Some(active) = self.filter.active() {
                self.scroll.scroll_to_item(active);
            }
        }
        div()
            .flex()
            .flex_col()
            // The query line stays outside the scroller: it is what the rows
            // are an answer to, and a reader who has scrolled away from it has
            // lost what they typed.
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
                    .into_any_element()
            } else {
                scroll::Viewport::new(
                    "search-results",
                    div()
                        .id("search-results-rows")
                        .max_h(px(MAX_ROWS * popover::menu_row_height()))
                        .flex()
                        .flex_col()
                        .children(rows),
                    Axis::Vertical,
                )
                .track_scroll(&self.scroll)
                .into_any_element()
            })
    }
}
