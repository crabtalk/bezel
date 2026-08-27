//! The rail, as its own view so it can be cached.
//!
//! Caching is per *view* in gpui, and it is opt-in for a reason: a cached
//! subtree is skipped entirely — not rendered, not laid out — until the entity
//! behind it is notified. The rail is the case that pays for it. It is forty
//! rows with a hover fade each, it changes only when a tab or a selection
//! changes, and it was being rebuilt on every frame of every drag and every
//! spinner tick.
//!
//! What it costs to opt in: this view holds the state it paints, so the page it
//! selects is reported to the host rather than reached into.

use gpui::{
    Context, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use motion::{Fade, Painter};
use theme::Theme;
use ui::{
    popover,
    scroll::{self, TransientState},
};

use crate::{RAIL_PAD, RAIL_WIDTH, TABS, Tab};

/// What the rail reports. The host owns which page is open — the rail only
/// says what was clicked, the way `tree` reports an intent it cannot carry out
/// itself.
pub struct Selected(pub &'static str);

impl EventEmitter<Selected> for Rail {}

pub struct Rail {
    tab: usize,
    selected: &'static str,
    scroll: gpui::ScrollHandle,
    bar: TransientState,
}

impl Rail {
    pub fn new(tab: usize, selected: &'static str, cx: &mut Context<Self>) -> Self {
        Self {
            tab,
            selected,
            scroll: gpui::ScrollHandle::new(),
            bar: TransientState::new(Painter::of(cx)),
        }
    }

    /// Point the rail at a tab and its open page. The notify is what busts the
    /// cache — without it the rail would paint the last tab forever.
    pub fn show(&mut self, tab: usize, selected: &'static str, cx: &mut Context<Self>) {
        if self.tab == tab && self.selected == selected {
            return;
        }
        self.tab = tab;
        self.selected = selected;
        cx.notify();
    }
}

impl Render for Rail {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let view = Painter::of(cx);
        let tab: &Tab = &TABS[self.tab];
        let selected = self.selected;

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id("gallery-rail")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .p(px(RAIL_PAD))
                                    .children(tab.groups.iter().flat_map(|group| {
                                        let heading = popover::menu_heading(&theme, group.title)
                                            .into_any_element();
                                        let rows = group.sections.iter().map(|section| {
                                            popover::menu_row(
                                                &theme,
                                                section.key == selected,
                                                Fade::new(view, format!("rail-{}", section.key)),
                                            )
                                            .id(SharedString::from(format!(
                                                "rail-item-{}",
                                                section.key
                                            )))
                                            .on_click(cx.listener(move |_, _, _, cx| {
                                                cx.emit(Selected(section.key));
                                            }))
                                            // Unbuilt rows stay legible but recede, so the rail
                                            // reads as "what exists" and "what is left" at once.
                                            .when(
                                                section.source.is_none() && section.key != selected,
                                                |row| row.text_color(theme.text_faint),
                                            )
                                            .child(SharedString::from(section.title))
                                            .into_any_element()
                                        });
                                        std::iter::once(heading).chain(rows)
                                    })),
                            ),
                    )
                    // After the content: hitboxes and paint are both
                    // order-dependent in gpui, so a bar added first would sit
                    // under what it reports on.
                    .child(scroll::transient(
                        "rail-bar",
                        &self.scroll,
                        &self.bar,
                        cx.reduce_motion(),
                    )),
            )
            .child(wordmark(&theme))
    }
}

fn wordmark(theme: &Theme) -> impl IntoElement {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(px(6.0))
        // Lines the wordmark up with the row labels above it: the rail's
        // padding plus `menu_row`'s own.
        .px(px(RAIL_PAD + 8.0))
        .py(px(10.0))
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child("bezel"),
        )
        .child(
            div()
                .text_size(px(11.0))
                .font_family(theme.font_mono.clone())
                .text_color(theme.text_faint)
                .child(env!("CARGO_PKG_VERSION")),
        )
}

/// The layout a cached rail is laid out at. Caching skips rendering the
/// contents, so the size cannot come from them.
pub fn style() -> gpui::StyleRefinement {
    gpui::StyleRefinement::default()
        .w(px(RAIL_WIDTH))
        .h_full()
        .flex_none()
}
