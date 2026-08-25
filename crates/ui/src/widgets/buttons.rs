//! The button — one component, three shipped looks. A catalog trait like
//! every widget group: `use ui::widgets::{ButtonStyle, Buttons};` →
//! `theme.button("Save", ButtonStyle::Prominent, None)`.
//!
//! [`ButtonStyle`] is a closed enum, not free-form knobs: it selects between
//! the looks that ship, while per-call overrides stay chain modifiers.

use gpui::{Div, SharedString, div, prelude::*, px};
use motion;
use theme::{Theme, ThemeExt, ink, wash};

/// The shipped looks (the reference `btnGhost` / `btnPrimary` /
/// `btnDestructive`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonStyle {
    /// Quiet text on a translucent wash.
    Ghost,
    /// The maximum-contrast plate — the primary action.
    Prominent,
    /// The muted red fill — carries the destructive semantics with the paint.
    Destructive,
}

/// The frame every style shares.
fn frame(mut el: Div) -> Div {
    el = el
        .px(px(12.0))
        .py(px(6.0))
        .rounded(px(Theme::button_radius()))
        .text_size(px(13.0))
        .cursor_pointer();
    el
}

pub trait Buttons: ThemeExt {
    /// A labeled button in one of the shipped styles. `fade_key` matters only
    /// for [`ButtonStyle::Ghost`]: `Some` animates the hover wash per instance
    /// (pass a unique key per button), `None` is the plain ghost with the
    /// hover left to the caller.
    fn button(
        &self,
        label: impl Into<SharedString>,
        style: ButtonStyle,
        fade_key: Option<SharedString>,
    ) -> Div {
        let theme = self.theme();
        let label = label.into();
        match style {
            ButtonStyle::Ghost => match fade_key {
                Some(fade_key) => {
                    let mut btn = frame(div())
                        .text_color(motion::hover_blend(&fade_key, theme.text_muted, theme.text))
                        .bg(motion::hover_blend(&fade_key, wash(0.0), ink(0.06)))
                        .child(label);
                    btn.interactivity()
                        .on_hover(motion::hover_listener(fade_key));
                    btn
                }
                None => frame(div()).text_color(theme.text_muted).child(label),
            },
            ButtonStyle::Prominent => frame(div())
                .bg(theme.text)
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.on_solid)
                .hover(|s| s.opacity(0.9))
                .child(label),
            ButtonStyle::Destructive => frame(div())
                .bg(theme.danger_strong)
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(gpui::white())
                .hover(|s| s.opacity(0.9))
                .child(label),
        }
    }
}

impl Buttons for Theme {}
