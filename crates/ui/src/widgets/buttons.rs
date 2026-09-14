//! The button — one component, three shipped looks — in its labelled and
//! glyph-only forms, the [`Buttons::control_group`] that gathers adjacent ones
//! onto one shared background, and [`Buttons::ghost`], the open frame a quiet
//! control paints around children of its own. A catalog trait like every widget
//! group: `use ui::widgets::{ButtonStyle, Buttons};` →
//! `theme.button("Save", ButtonStyle::Prominent, None)`.
//!
//! [`ButtonStyle`] is a closed enum, not free-form knobs: it selects between
//! the looks that ship, while per-call overrides stay chain modifiers.

use gpui::{Div, ElementId, SharedString, Stateful, div, prelude::*, px};
use icons::Icon;
use motion::{self, Fade};
use theme::{ControlSize, Sizing, Theme, ThemeExt};

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

/// The glyph a [`Buttons::icon_button`] carries, at the size every other
/// control in this crate paints one.
const GLYPH: f32 = 14.0;

/// What a [`Buttons::control_group`] insets its items by, and the gap between
/// them — so the first sits as far from the track's edge as from its neighbour.
const GROUP_PAD: f32 = 2.0;

/// The frame every style shares — [`ControlSize::Regular`], which a caller
/// moves with [`Sizing::control_size`].
fn frame() -> Div {
    div()
        .control_size(ControlSize::Regular)
        .flex()
        .items_center()
        .cursor_pointer()
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
        let (button, _) = appearance(self.theme(), frame(), style, None, fade, true);
        button.child(label.into())
    }

    /// A button that is only a glyph — SwiftUI's toolbar `Button` over an icon
    /// `Label`. Square at [`Theme::BUTTON_HEIGHT`], so it stands the same
    /// height as a [`Self::button`] beside it. `fade` reads as it does there.
    ///
    /// It builds the glyph rather than taking one: gpui reads an svg's colour
    /// off that element's own style and paints **nothing** when it is unset, so
    /// a colour set on this button would silently not reach it.
    ///
    /// An icon carries no accessible name — reach for
    /// [`crate::tooltip`] on the way past.
    fn icon_button(&self, icon: impl Into<Icon>, style: ButtonStyle, fade: Option<Fade>) -> Div {
        let square = frame()
            .px(px(0.0))
            .w(px(Theme::BUTTON_HEIGHT))
            .justify_center();
        let (button, tint) = appearance(self.theme(), square, style, None, fade, true);
        button.child(crate::icons::icon(icon).size(px(GLYPH)).text_color(tint))
    }

    /// SwiftUI's `ControlGroup`, and what a toolbar paints behind the items it
    /// finds side by side: one shared background, its buttons inset in it. A
    /// second cluster is a second call — the break between them is the spacing,
    /// the way `ToolbarSpacer` puts it there.
    ///
    /// The track's radius is the item's plus the inset, so an ordinary
    /// [`Self::button`] or [`Self::icon_button`] drops in already concentric.
    ///
    /// Items are left to stretch: that is what holds a glyph and a label to one
    /// height when the type ladder moves under them. `self_start` because the
    /// group must hug them — dropped into a `flex_col`, flexbox's default
    /// `align-items: stretch` would otherwise blow it out to the column's full
    /// width.
    ///
    /// Glass is chained, not baked: `.surface(theme, theme.popover_surface)`
    /// turns the track into the capsule a macOS 26 toolbar floats.
    fn control_group(&self) -> Div {
        let theme = self.theme();
        div()
            .self_start()
            .flex()
            .flex_row()
            .gap(px(GROUP_PAD))
            .p(px(GROUP_PAD))
            .rounded(px(Theme::button_radius() + GROUP_PAD))
            .bg(theme.surface_raised)
            .border_1()
            .border_color(theme.border)
    }

    /// A quiet control: nothing at rest, a wash on hover. Stateful, so it
    /// carries its own click and tooltip; padding and children are the
    /// caller's, which is what lets a glyph sit before the text.
    fn ghost(&self, id: impl Into<ElementId>) -> Stateful<Div> {
        let tint = self.theme().element_hover;
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

/// Purpose is independent of emphasis: a destructive action may be quiet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonRole {
    Cancel,
    Destructive,
}

pub(super) fn appearance(
    theme: &Theme,
    frame: Div,
    style: ButtonStyle,
    role: Option<ButtonRole>,
    fade: Option<Fade>,
    interactive: bool,
) -> (Div, gpui::Hsla) {
    let destructive = role == Some(ButtonRole::Destructive) || style == ButtonStyle::Destructive;
    if style == ButtonStyle::Ghost {
        let rest = if destructive {
            theme.danger_strong
        } else {
            theme.text_muted
        };
        let hot = if destructive {
            theme.danger_strong
        } else {
            theme.text
        };
        if let Some(fade) = fade {
            let tint = motion::hover_blend(&fade, rest, hot);
            let mut button = frame.text_color(tint).bg(motion::hover_blend(
                &fade,
                theme.ink(0.0),
                theme.element_hover,
            ));
            button
                .interactivity()
                .on_hover(motion::hover_listener(fade));
            (button, tint)
        } else {
            (frame.text_color(rest), rest)
        }
    } else {
        let (fill, tint) = if destructive {
            (theme.danger_strong, gpui::white())
        } else {
            (theme.text, theme.on_solid)
        };
        (
            frame
                .bg(fill)
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(tint)
                .when(interactive, |button| button.hover(|s| s.opacity(0.9))),
            tint,
        )
    }
}
