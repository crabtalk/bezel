//! Content pieces — badges, avatar, icon tiles and decks, tag, breadcrumb, the
//! empty state.
//!
//! A catalog trait, like every widget group: import it to unlock
//! `theme.badge(..)`, `theme.avatar(..)`, `theme.empty_state(..)`.

use gpui::{Div, SharedString, Svg, div, prelude::*, px};
use icons::Icon;
use theme::{TextStyle, Theme, ThemeExt, Typeset};

/// The tiles an [`Content::icon_deck`] draws behind its front one, at most.
const DECK_DEPTH: usize = 2;
/// How far each tile behind sticks out past the one before it.
const DECK_PEEK: f32 = 4.0;
/// How much shorter each tile behind is, top and bottom.
const DECK_INSET: f32 = 2.5;
/// The ink each tile behind is washed in, nearest first.
const DECK_WASH: [f32; DECK_DEPTH] = [0.14, 0.07];
/// The room between a tile's edge and its icon.
const TILE_PAD: f32 = 4.0;

pub trait Content: ThemeExt {
    /// Right-anchored badge pill: `rounded-full border px-2 py-0.5`.
    fn badge(&self, label: impl Into<SharedString>) -> Div {
        let theme = self.theme();
        div()
            .flex_none()
            .px(px(8.0))
            .py(px(2.0))
            .rounded_full()
            .border_1()
            .border_color(theme.border)
            .text_style(TextStyle::Caption)
            .text_color(theme.text_muted)
            .child(label.into())
    }

    /// Emerald status pill (the Accounts "Active" badge:
    /// `bg-emerald-400/[0.12] text-emerald-300/90`).
    fn badge_active(&self, label: impl Into<SharedString>) -> Div {
        let theme = self.theme();
        let emerald = theme.success;
        let emerald_text = theme.success_muted; // emerald-300
        div()
            .flex_none()
            .px(px(8.0))
            .py(px(2.0))
            .rounded_full()
            .bg(emerald.opacity(0.12))
            .text_style(TextStyle::Caption)
            .text_color(emerald_text.opacity(0.9))
            .child(label.into())
    }

    /// Circular avatar holding one or two initials — the fallback every avatar
    /// needs, and often the whole thing on a monochrome surface. Initials only:
    /// there is no image variant.
    fn avatar(&self, initials: impl Into<SharedString>) -> Div {
        let theme = self.theme();
        div()
            .flex_none()
            .size(px(28.0))
            .rounded_full()
            .bg(theme.ink(0.12))
            .flex()
            .items_center()
            .justify_center()
            .text_style(TextStyle::Subheadline)
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme.text_muted)
            .child(initials.into())
    }

    /// An icon on a square tile, the tile the same whatever the icon's own
    /// shape — so marks of different shapes line up, and overlap as cards in
    /// [`Self::icon_deck`].
    fn icon_tile(&self, icon: impl Into<Icon>) -> Div {
        let theme = self.theme();
        tile()
            .bg(theme.surface_raised)
            .border_1()
            .border_color(theme.border_faint)
            .child(
                crate::icons::icon(icon)
                    .size(px(TextStyle::Subheadline.painted()))
                    .text_color(theme.text),
            )
    }

    /// A stack shown as a deck: `front`'s [`Self::icon_tile`], with the edges of
    /// blank tiles peeking out behind it, one for each of `behind` up to two,
    /// each fainter than the one before.
    /// More than two behind draws as two. Always as wide as a full deck, so a
    /// row holding one does not shift as the stack grows or shrinks.
    fn icon_deck(&self, front: impl Into<Icon>, behind: usize) -> Div {
        let theme = self.theme();
        let edge = tile_edge();
        div()
            .flex_none()
            .relative()
            .w(px(edge + DECK_PEEK * DECK_DEPTH as f32))
            .h(px(edge))
            .children((1..=behind.min(DECK_DEPTH)).rev().map(|at| {
                let inset = DECK_INSET * at as f32;
                tile()
                    .bg(theme.ink(DECK_WASH[at - 1]))
                    .absolute()
                    .left(px(DECK_PEEK * at as f32))
                    .top(px(inset))
                    .h(px(edge - inset * 2.))
            }))
            .child(self.icon_tile(front).absolute().left_0().top_0())
    }

    /// A removable chip — a token in a filter bar or a recipient field. The
    /// caller adds the click handler for the ✕.
    fn tag(&self, label: impl Into<SharedString>) -> Div {
        let theme = self.theme();
        div()
            .self_start()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(5.0))
            .pl(px(8.0))
            .pr(px(5.0))
            .py(px(3.0))
            .rounded(px(Theme::control_radius()))
            .bg(theme.ink(0.07))
            .border_1()
            .border_color(theme.border)
            .text_style(TextStyle::Callout)
            .text_color(theme.text)
            .child(label.into())
            .child(
                crate::icons::icon(crate::icons::glyph::X)
                    .size(px(10.0))
                    .text_color(theme.text_faint),
            )
    }

    /// A name in a colour — a label on an entry, an option of a
    /// multi-select. `tint` is the hue it is told apart by, painted as a wash
    /// under the theme's text; [`crate::multi_select::tint`] derives one from
    /// the name. A long name ends in `…`; the caller caps the width.
    fn chip(&self, label: impl Into<SharedString>, tint: gpui::Hsla) -> Div {
        let theme = self.theme();
        div()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .px(px(6.0))
            .rounded(px(Theme::control_radius()))
            .bg(tint.opacity(0.22))
            .text_style(TextStyle::Callout)
            .text_color(theme.text)
            .child(div().min_w_0().truncate().child(label.into()))
    }

    /// Breadcrumb trail. Items are added by the caller with
    /// [`Self::breadcrumb_item`], separated by [`Self::breadcrumb_separator`].
    fn breadcrumb(&self) -> Div {
        div()
            .self_start()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .min_w_0()
    }

    /// One crumb. The last one is `current` and stops looking clickable.
    fn breadcrumb_item(&self, label: impl Into<SharedString>, current: bool) -> Div {
        let theme = self.theme();
        let mut crumb = div()
            .min_w_0()
            .truncate()
            .text_style(TextStyle::Callout)
            .child(label.into());
        crumb = if current {
            crumb.text_color(theme.text)
        } else {
            crumb.text_color(theme.text_muted).cursor_pointer()
        };
        crumb
    }

    /// The chevron between crumbs.
    fn breadcrumb_separator(&self) -> Svg {
        let theme = self.theme();
        crate::icons::icon(crate::icons::glyph::ChevronRight)
            .size(px(12.0))
            .text_color(theme.text_faint)
    }

    /// The centered "nothing here yet" panel: icon, headline, one line of hint.
    fn empty_state(
        &self,
        icon: impl Into<Icon>,
        title: impl Into<SharedString>,
        hint: impl Into<SharedString>,
    ) -> Div {
        let theme = self.theme();
        div()
            .w_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .py(px(40.0))
            .child(
                crate::icons::icon(icon)
                    .size(px(24.0))
                    .text_color(theme.text_faint),
            )
            .child(
                div()
                    .text_style(TextStyle::Headline)
                    .text_color(theme.text)
                    .child(title.into()),
            )
            .child(
                div()
                    .text_style(TextStyle::Callout)
                    .text_color(theme.text_muted)
                    .child(hint.into()),
            )
    }
}

impl Content for Theme {}

/// The side of an [`Content::icon_tile`]: a body-sized square and its pad.
fn tile_edge() -> f32 {
    TextStyle::Body.painted() + TILE_PAD * 2.
}

/// A tile's shape, [`tile_edge`] square and unpainted — what
/// [`Content::icon_tile`] puts its icon on and [`Content::icon_deck`] stacks
/// behind it.
fn tile() -> Div {
    let edge = tile_edge();
    div()
        .flex_none()
        .w(px(edge))
        .h(px(edge))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(Theme::control_radius()))
}
