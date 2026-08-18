//! The page skeleton — column, header, section cards, rows — the shared
//! rhythm of the settings pages (the reference settings.devices.tsx /
//! settings.agents.tsx / settings.archived.tsx).
//!
//! A catalog trait, like every widget group: import it to unlock
//! `theme.group_box()`, `theme.page_header(..)`, `theme.card_row(..)`.
//! Extends [`ThemeExt`], which carries the environment; the `Theme` impl is
//! empty because every method below has a default. Not object-safe — its
//! methods are statically dispatched onto [`Theme`].

use gpui::{AnyElement, Div, SharedString, div, prelude::*, px};
use theme::{Theme, ThemeExt, ink};

/// Default height of an [`Scaffolding::option_card`] preview frame.
pub const OPTION_CARD_HEIGHT: f32 = 148.0;
/// Corner radius of the preview frame.
///
/// Public because the preview has to round *itself* to this. gpui content masks
/// are axis-aligned rectangles, so `overflow_hidden` on the frame clips to its
/// bounding box and not to its corner radius — a preview that paints its own
/// background will square off the corners and cover the frame's border with it.
pub const OPTION_CARD_RADIUS: f32 = 10.0;
/// Clear space between the frame and the selection ring.
const RING_GAP: f32 = 2.0;
/// Thickness of the selection ring.
const RING_WIDTH: f32 = 2.0;

pub trait Scaffolding: ThemeExt {
    /// Centered page column: `mx-auto w-full max-w-3xl px-6 pb-16 pt-8`.
    fn page_column(&self) -> Div {
        div()
            .w_full()
            .max_w(px(768.0))
            .mx_auto()
            .px(px(24.0))
            .pt(px(32.0))
            .pb(px(64.0))
            .flex()
            .flex_col()
    }

    /// Page headline row: `flex items-baseline gap-2.5` — `text-base font-semibold`
    /// title + `text-[13px]` count sharing a baseline (the reference settings.devices.tsx).
    fn page_header(&self, title: impl Into<SharedString>, count: Option<usize>) -> Div {
        let theme = self.theme();
        div()
            .flex()
            .flex_row()
            .items_baseline()
            .gap(px(10.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child(title.into()),
            )
            .when_some(count, |el, count| {
                el.child(
                    div()
                        .text_size(px(13.0))
                        .text_color(theme.text_muted.opacity(0.7))
                        .child(format!("{count}")),
                )
            })
    }

    /// Subtitle under the headline: `mt-1 text-[13px] text-muted-foreground`.
    fn page_subtitle(&self, copy: impl Into<SharedString>) -> Div {
        let theme = self.theme();
        div()
            .mt(px(4.0))
            .text_size(px(13.0))
            .text_color(theme.text_muted)
            .child(copy.into())
    }

    /// Small label above a group of controls (`text-[13px] font-medium`) — the
    /// "Theme" caption over a picker, not a page headline.
    fn field_label(&self, label: impl Into<SharedString>) -> Div {
        let theme = self.theme();
        div()
            .text_size(px(13.0))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme.text)
            .child(label.into())
    }

    /// A row of equally-sized preview cards for picking one of N *visual* options.
    ///
    /// Deliberately knows nothing about themes: the caller supplies each preview as
    /// an arbitrary element and picks however many cards it wants, so the same
    /// control works for a density picker, a layout picker or anything else where
    /// the choice is easier to show than to describe. Pair with [`Self::option_card`].
    fn option_card_row(&self) -> Div {
        div().flex().flex_row().items_start().gap(px(16.0)).w_full()
    }

    /// One card in an [`Self::option_card_row`]: a fixed-height preview frame
    /// that carries the selection ring, with a caption underneath.
    ///
    /// `preview` fills the frame and **must round its own corners** to
    /// [`OPTION_CARD_RADIUS`] if it paints a background — see that constant.
    ///
    /// Returns a plain `Div` like the rest of this module — the caller adds
    /// `.id(..)` and `.on_click(..)`, so selection behaviour stays with the
    /// page that owns the state.
    fn option_card(
        &self,
        label: impl Into<SharedString>,
        selected: bool,
        preview: AnyElement,
    ) -> Div {
        let theme = self.theme();
        let frame = div()
            .h(px(OPTION_CARD_HEIGHT))
            .w_full()
            .rounded(px(OPTION_CARD_RADIUS))
            .overflow_hidden()
            .border_1()
            .border_color(theme.border)
            .child(preview);

        // The ring is a *wrapper border*, not a spread shadow. A shadow's spread
        // grows the rectangle without growing its corner radius, so the halo's
        // corners tighten relative to the frame's and the two visibly drift apart by
        // a pixel at each rounded corner. Concentric borders can't do that: each
        // element rounds itself, and the outer radius is the inner one plus the gap
        // it sits behind. Always present, transparent when unselected, so selecting a
        // card never reflows the row.
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .child(
                div()
                    .w_full()
                    .rounded(px(OPTION_CARD_RADIUS + RING_GAP + RING_WIDTH))
                    .p(px(RING_GAP))
                    .border_2()
                    .border_color(if selected {
                        theme.accent
                    } else {
                        gpui::transparent_black()
                    })
                    .child(frame),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(if selected {
                        theme.text
                    } else {
                        theme.text_muted
                    })
                    .child(label.into()),
            )
    }

    /// Section card: `mt-6 overflow-hidden rounded-xl border border-border bg-card`
    /// — the card tone, thinned to a translucent tint over glass so the card
    /// reads as frost instead of a solid slab ([`Theme::card_glass_bg`]).
    fn group_box(&self) -> Div {
        let theme = self.theme();
        div()
            .mt(px(24.0))
            .rounded(px(Theme::SURFACE_RADIUS))
            .border_1()
            .border_color(theme.border)
            .bg(theme.card_glass_bg())
            .overflow_hidden()
            .flex()
            .flex_col()
    }

    /// One card row: `border-t border-border px-5 py-3.5 first:border-t-0`.
    /// Hover is caller-owned — gpui panics on a second hover, so the default
    /// wash is [`super::card_row_hover`] for the caller to chain.
    fn card_row(&self, first: bool) -> Div {
        let theme = self.theme();
        div()
            .px(px(20.0))
            .py(px(14.0))
            .when(!first, |el| el.border_t_1().border_color(theme.border))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(14.0))
    }

    /// The identity tile on a row: `size-9 rounded-[10px] border bg-white/[0.03]`
    /// around a 16px icon.
    fn row_tile(&self, icon_path: &'static str) -> Div {
        let theme = self.theme();
        div()
            .flex_none()
            .size(px(36.0))
            .rounded(px(Theme::PANEL_RADIUS))
            .border_1()
            .border_color(theme.border)
            .bg(ink(0.03))
            .flex()
            .items_center()
            .justify_center()
            .child(
                crate::icons::icon(icon_path)
                    .size(px(16.0))
                    .text_color(theme.text_muted),
            )
    }

    /// Row title: `text-[13.5px] font-medium leading-tight`.
    fn row_title(&self, title: impl Into<SharedString>) -> Div {
        let theme = self.theme();
        div()
            .min_w_0()
            .truncate()
            .text_size(px(13.5))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme.text)
            .child(title.into())
    }

    /// The quiet meta line under a row title: `text-[11.5px]
    /// text-muted-foreground/65` fragments joined by dots.
    fn meta_line(&self, fragments: Vec<AnyElement>) -> Div {
        let theme = self.theme();
        let mut line = div()
            .mt(px(4.0))
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_x(px(8.0))
            .gap_y(px(2.0))
            .text_size(px(11.5))
            .text_color(theme.text_muted.opacity(0.65));
        let mut first = true;
        for fragment in fragments {
            if !first {
                line = line.child(
                    div()
                        .text_color(theme.text_muted.opacity(0.3))
                        .child(SharedString::from("·")),
                );
            }
            line = line.child(fragment);
            first = false;
        }
        line
    }
}

impl Scaffolding for Theme {}
