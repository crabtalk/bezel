//! [`Tooltip`] — the small hover label.
//!
//! An entity rather than a function because gpui's `.tooltip(..)` takes a
//! builder returning an `AnyView`: the tooltip is mounted in its own layer,
//! after the hover delay, so it cannot be an inline element.
//!
//! ```ignore
//! use bezel_ui::tooltip::Tooltip;
//!
//! div()
//!     .id("copy")
//!     .tooltip(|window, cx| Tooltip::text("Copy path", window, cx))
//!     .child("⌘C")
//! ```

use gpui::{AnyView, App, Context, IntoElement, SharedString, Window, div, prelude::*, px};

use bezel_theme::Theme;

use crate::popover;

pub struct Tooltip {
    text: SharedString,
    /// Optional keystroke shown right-aligned, e.g. `⌘C`.
    keystroke: Option<SharedString>,
}

impl Tooltip {
    /// A plain text tooltip, built for `.tooltip(..)`.
    pub fn text(text: impl Into<SharedString>, _window: &mut Window, cx: &mut App) -> AnyView {
        let text = text.into();
        cx.new(|_| Self {
            text,
            keystroke: None,
        })
        .into()
    }

    /// A tooltip that also names the shortcut — the pairing that keeps
    /// keyboard affordances discoverable without a menu.
    pub fn with_keystroke(
        text: impl Into<SharedString>,
        keystroke: impl Into<SharedString>,
        _window: &mut Window,
        cx: &mut App,
    ) -> AnyView {
        let (text, keystroke) = (text.into(), keystroke.into());
        cx.new(|_| Self {
            text,
            keystroke: Some(keystroke),
        })
        .into()
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        // Tooltips are small and frequent, so this is a tighter card than
        // `popover_card`: less padding, no menu rhythm.
        popover::popover_card(&theme)
            .px(px(8.0))
            .py(px(5.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .text_size(px(12.0))
            .text_color(theme.text)
            .child(self.text.clone())
            .when_some(self.keystroke.clone(), |card, keystroke| {
                card.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme.text_faint)
                        .child(keystroke),
                )
            })
    }
}
