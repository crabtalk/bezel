//! [`Tooltip`] — the small hover label.
//!
//! An entity rather than a function because gpui's `.tooltip(..)` takes a
//! builder returning an `AnyView`: the tooltip is mounted in its own layer,
//! after the hover delay, so it cannot be an inline element.
//!
//! ```ignore
//! use ui::tooltip::Tooltip;
//!
//! div()
//!     .id("copy")
//!     .tooltip(|window, cx| Tooltip::text("Copy path", window, cx))
//!     .child("⌘C")
//! ```

use gpui::{Action, AnyView, App, Context, IntoElement, SharedString, Window, div, prelude::*, px};

use theme::{TextStyle, Theme, Typeset};

use crate::{keys, popover, surface::Surfaced as _};

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

    /// The same pairing with the chord read off the keymap rather than typed
    /// in, so rebinding the action moves the hint with it. Falls back to plain
    /// text when nothing is bound.
    ///
    /// A tooltip is built on hover, while the surface it describes still holds
    /// focus, so [`keys::shortcut`] is the right lookup — use
    /// [`Tooltip::for_action_in`] where it is not, such as a button that moves
    /// focus to itself.
    pub fn for_action(
        text: impl Into<SharedString>,
        action: &dyn Action,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyView {
        let text = text.into();
        let keystroke = keys::shortcut(action, window);
        cx.new(|_| Self { text, keystroke }).into()
    }

    /// [`Tooltip::for_action`] against a named key context instead of whatever
    /// holds focus.
    pub fn for_action_in(
        text: impl Into<SharedString>,
        action: &dyn Action,
        context: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyView {
        let text = text.into();
        let keystroke = keys::shortcut_in(action, context, window);
        cx.new(|_| Self { text, keystroke }).into()
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
            .gap(px(Theme::SPACE))
            .text_style(TextStyle::Callout)
            .text_color(theme.text)
            .child(self.text.clone())
            .when_some(self.keystroke.clone(), |card, keystroke| {
                card.child(
                    div()
                        .text_style(TextStyle::Subheadline)
                        .text_color(theme.text_faint)
                        .child(keystroke),
                )
            })
            .surface(&theme, theme.popover_surface)
    }
}
