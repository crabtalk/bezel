//! The button — one component, three shipped looks — and [`Buttons::ghost`],
//! the frame a control paints when it is only a glyph. A catalog trait like
//! every widget group: `use ui::widgets::{ButtonStyle, Buttons};` →
//! `theme.button("Save", ButtonStyle::Prominent, None)`.
//!
//! [`ButtonStyle`] is a closed enum, not free-form knobs: it selects between
//! the looks that ship, while per-call overrides stay chain modifiers.

use gpui::{Div, ElementId, SharedString, Stateful, div, prelude::*, px};
use motion::{self, Fade};
use theme::{TextStyle, Theme, ThemeExt, Typeset, ink, wash};

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
        .text_style(TextStyle::Body)
        .cursor_pointer();
    el
}

pub trait Buttons: ThemeExt {
    /// A labeled button in one of the shipped styles. `fade` matters only for
    /// [`ButtonStyle::Ghost`]: `Some` animates the hover wash per instance,
    /// `None` is the plain ghost with the hover left to the caller.
    fn button(
        &self,
        label: impl Into<SharedString>,
        style: ButtonStyle,
        fade: Option<Fade>,
    ) -> Div {
        let theme = self.theme();
        let label = label.into();
        match style {
            ButtonStyle::Ghost => match fade {
                Some(fade) => {
                    let mut btn = frame(div())
                        .text_color(motion::hover_blend(&fade, theme.text_muted, theme.text))
                        .bg(motion::hover_blend(&fade, wash(0.0), ink(0.06)))
                        .child(label);
                    btn.interactivity().on_hover(motion::hover_listener(fade));
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

    /// A quiet control: nothing at rest, a wash on hover. Stateful, so it
    /// carries its own click and tooltip; padding and children are the
    /// caller's, which is what lets a glyph sit before the text.
    fn ghost(&self, id: impl Into<ElementId>) -> Stateful<Div> {
        let tint = self.theme().glass_hover();
        div()
            .id(id)
            .flex()
            .flex_row()
            .items_center()
            .rounded(px(Theme::control_radius()))
            .cursor_pointer()
            .hover(move |el| el.bg(tint))
    }
}

impl Buttons for Theme {}
