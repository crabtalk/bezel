//! [`HoverCard`] — the richer sibling of [`crate::tooltip::Tooltip`]: a card
//! that opens on hover and can itself be hovered, for a preview the pointer
//! can travel into (a profile, a link target, a definition).
//!
//! It is mounted through `hoverable_tooltip`, not `tooltip` — that one word is
//! the whole difference between a tooltip and a hover card, and it means there
//! is no open/close state machine to write here: gpui already owns the delay
//! (500ms by default, `.tooltip_show_delay(..)` to change it) and keeps the
//! card alive while the pointer is inside it.
//!
//! ```ignore
//! use ui::hover_card::HoverCard;
//!
//! div()
//!     .id("clearloop")
//!     .hoverable_tooltip(|window, cx| {
//!         HoverCard::summary("clearloop", "Builds desktop software in Rust.", window, cx)
//!     })
//!     .child("@clearloop")
//! ```

use gpui::{AnyView, App, Context, IntoElement, SharedString, Window, div, prelude::*, px};

use crate::widgets::Content;
use theme::Theme;

use crate::popover;

pub struct HoverCard {
    title: SharedString,
    body: SharedString,
    /// Initials for the leading avatar, when the subject is a person.
    initials: Option<SharedString>,
    /// The quiet line under the body — a role, a path, a timestamp.
    meta: Option<SharedString>,
}

impl HoverCard {
    /// A plain card: heading plus a line or two of prose.
    pub fn summary(
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        _window: &mut Window,
        cx: &mut App,
    ) -> AnyView {
        let (title, body) = (title.into(), body.into());
        cx.new(|_| Self {
            title,
            body,
            initials: None,
            meta: None,
        })
        .into()
    }

    /// A hover card for a person: avatar initials beside the name, and a meta
    /// line under the body.
    pub fn person(
        initials: impl Into<SharedString>,
        name: impl Into<SharedString>,
        body: impl Into<SharedString>,
        meta: impl Into<SharedString>,
        _window: &mut Window,
        cx: &mut App,
    ) -> AnyView {
        let (initials, name, body, meta) = (initials.into(), name.into(), body.into(), meta.into());
        cx.new(|_| Self {
            title: name,
            body,
            initials: Some(initials),
            meta: Some(meta),
        })
        .into()
    }
}

impl Render for HoverCard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let heading = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .when_some(self.initials.clone(), |row, initials| {
                row.child(theme.avatar(initials))
            })
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(self.title.clone()),
            );

        // Wider and airier than a tooltip: this holds prose, not a label.
        popover::popover_card(&theme)
            .w(px(280.0))
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(heading)
            .child(
                div()
                    .text_size(px(12.5))
                    .line_height(px(18.0))
                    .text_color(theme.text_muted)
                    .child(self.body.clone()),
            )
            .when_some(self.meta.clone(), |card, meta| {
                card.child(
                    div()
                        .text_size(px(11.5))
                        .text_color(theme.text_faint)
                        .child(meta),
                )
            })
    }
}
