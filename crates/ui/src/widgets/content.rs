//! Content pieces — badges, avatar, tag, breadcrumb, the empty state.
//!
//! A catalog trait, like every widget group: import it to unlock
//! `theme.badge(..)`, `theme.avatar(..)`, `theme.empty_state(..)`.

use bezel_theme::{Theme, ThemeExt, ink};
use gpui::{Div, SharedString, Svg, div, prelude::*, px};

pub trait Content: ThemeExt {
    /// Right-anchored badge pill: `rounded-full border px-2 py-0.5 text-[10.5px]`.
    fn badge(&self, label: impl Into<SharedString>) -> Div {
        let theme = self.theme();
        div()
            .flex_none()
            .px(px(8.0))
            .py(px(2.0))
            .rounded_full()
            .border_1()
            .border_color(theme.border)
            .text_size(px(10.5))
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
            .text_size(px(10.5))
            .text_color(emerald_text.opacity(0.9))
            .child(label.into())
    }

    /// Circular avatar holding one or two initials — the fallback every avatar
    /// needs, and often the whole thing on a monochrome surface.
    fn avatar(&self, initials: impl Into<SharedString>) -> Div {
        let theme = self.theme();
        div()
            .flex_none()
            .size(px(28.0))
            .rounded_full()
            .bg(ink(0.12))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(11.0))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme.text_muted)
            .child(initials.into())
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
            .rounded(px(Theme::CONTROL_RADIUS))
            .bg(ink(0.07))
            .border_1()
            .border_color(theme.border)
            .text_size(px(12.0))
            .text_color(theme.text)
            .child(label.into())
            .child(
                crate::icons::icon(crate::icons::CLOSE)
                    .size(px(10.0))
                    .text_color(theme.text_faint),
            )
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
            .text_size(px(12.5))
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
        crate::icons::icon(crate::icons::ALT_ARROW_RIGHT)
            .size(px(12.0))
            .text_color(theme.text_faint)
    }

    /// The centered "nothing here yet" panel: icon, headline, one line of hint.
    fn empty_state(
        &self,
        icon_path: &'static str,
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
                crate::icons::icon(icon_path)
                    .size(px(24.0))
                    .text_color(theme.text_faint),
            )
            .child(
                div()
                    .text_size(px(13.5))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(title.into()),
            )
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(theme.text_muted)
                    .child(hint.into()),
            )
    }
}

impl Content for Theme {}
