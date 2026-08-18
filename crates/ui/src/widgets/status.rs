//! Status surfaces — the step row and its output, and the alert strips.
//!
//! A catalog trait, like every widget group: import it to unlock
//! `theme.step_row(..)`, `theme.step_output(..)`, `theme.error_strip(..)`.

use gpui::{Div, SharedString, div, prelude::*, px};
use theme::{Theme, ThemeExt};

use crate::widgets::Layout;

/// Padding inside a [`Status::step_row`] and the [`Status::step_output`] under
/// it — the two have to agree or the output's first character sits left of the
/// title.
const STEP_PAD_X: f32 = 10.0;
const STEP_PAD_Y: f32 = 6.0;
/// How tall an output may get before it scrolls (`ToolCard.svelte`'s
/// `max-h-64`).
const STEP_OUTPUT_MAX: f32 = 256.0;

pub trait Status: ThemeExt {
    /// One operation, as a row: an icon, what it was, and how it went.
    ///
    /// A tool call in a transcript, a step in a CI run, a file in a migration —
    /// the shape is the same everywhere, which is why this takes strings and
    /// not a type that knows what any of them mean. `detail` is the truncating
    /// middle (a query, a path, a `· 3` count), `meta` the right-aligned figure
    /// that never truncates (a duration, a size, a row count).
    ///
    /// `expanded` is `None` when there is nothing under the row, and the
    /// chevron is simply absent — a disclosure that opens onto nothing is worse
    /// than no disclosure. `Some` renders it, and the caller owns the flag.
    ///
    /// Returns a plain `Div` like the rest of this module: the caller adds
    /// `.id(..)` and `.on_click(..)` **to this row**, never to a wrapper around
    /// it, or the hitbox ends up narrower than what it paints. Hover is
    /// caller-owned (gpui panics on a second hover); the default wash is
    /// [`step_row_hover`](crate::widgets::step_row_hover).
    fn step_row(
        &self,
        icon: &'static str,
        title: impl Into<SharedString>,
        detail: Option<SharedString>,
        meta: Option<SharedString>,
        failed: bool,
        expanded: Option<bool>,
    ) -> Div {
        let theme = self.theme();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .px(px(STEP_PAD_X))
            .py(px(STEP_PAD_Y))
            .cursor_pointer()
            .child(
                // Tinted here rather than on the row: gpui reads an svg's colour
                // off its own style, so a colour set on the parent never arrives.
                crate::icons::icon(icon)
                    .size(px(14.0))
                    .text_color(if failed {
                        theme.danger
                    } else {
                        theme.text_muted
                    }),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(px(12.5))
                    .text_color(theme.text)
                    .child(title.into()),
            )
            .when_some(detail, |row, detail| {
                row.child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.0))
                        .text_color(theme.text_muted)
                        .child(detail),
                )
            })
            .child(
                div()
                    .ml_auto()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .when_some(meta, |cluster, meta| {
                        cluster.child(
                            div()
                                .font_family(theme.font_mono.clone())
                                .text_size(px(12.0))
                                .text_color(theme.text_muted)
                                .child(meta),
                        )
                    })
                    .when_some(expanded, |cluster, expanded| {
                        cluster.child(Layout::disclosure(theme, expanded))
                    }),
            )
    }

    /// What a [`Self::step_row`] opens onto: its output, verbatim.
    ///
    /// Monospaced and capped, because the thing being shown is a program's
    /// stdout and the row it hangs off is one line tall — a 900-line stack
    /// trace pushing the next step off screen is the failure this cap exists
    /// for. Past the cap it scrolls, which is why it takes an id.
    ///
    /// No scrollbar: the wheel reaches it regardless, and a bar would need a
    /// `ScrollHandle` and a `ScrollbarState` from every caller for a box that
    /// is usually four lines long. Wrap it in `div().relative()` with
    /// [`crate::scroll::scrollbar`] over it if a particular one earns the bar.
    fn step_output(
        &self,
        id: impl Into<gpui::ElementId>,
        text: impl Into<SharedString>,
    ) -> gpui::Stateful<Div> {
        let theme = self.theme();
        div()
            .id(id)
            .max_h(px(STEP_OUTPUT_MAX))
            .overflow_y_scroll()
            .border_t_1()
            .border_color(theme.border)
            .px(px(STEP_PAD_X))
            .py(px(STEP_PAD_Y))
            .font_family(theme.font_mono.clone())
            .text_size(px(12.0))
            .text_color(theme.text_muted)
            .child(text.into())
    }

    /// The dismissible red error strip (`flex items-start gap-2 rounded-xl
    /// border border-red-400/20 bg-red-400/[0.06] text-red-300/90` with a
    /// leading `DangerTriangle mt-0.5 size-4`).
    fn error_strip(&self, message: impl Into<SharedString>) -> Div {
        let theme = self.theme();
        let red = theme.danger; // red-400
        let red_text = theme.danger_muted; // red-300
        div()
            .mt(px(16.0))
            .px(px(16.0))
            .py(px(12.0))
            .rounded(px(Theme::SURFACE_RADIUS))
            .border_1()
            .border_color(red.opacity(0.2))
            .bg(red.opacity(0.06))
            .text_size(px(12.5))
            .text_color(red_text.opacity(0.9))
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.0))
            .child(
                div().flex_none().mt(px(2.0)).child(
                    crate::icons::icon(crate::icons::DANGER_TRIANGLE)
                        .size(px(16.0))
                        .text_color(red_text.opacity(0.9)),
                ),
            )
            .child(div().min_w_0().child(message.into()))
    }

    /// The amber warning strip (`flex items-start gap-2 border-amber-400/20
    /// bg-amber-400/[0.06] text-amber-200/90` with a leading `DangerTriangle
    /// mt-0.5 size-3.5`).
    fn warning_strip(&self, message: impl Into<SharedString>) -> Div {
        let theme = self.theme();
        let amber = theme.warning; // amber-400
        let amber_text = theme.warning_muted; // amber-200
        div()
            .mt(px(8.0))
            .px(px(16.0))
            .py(px(10.0))
            .rounded(px(Theme::SURFACE_RADIUS))
            .border_1()
            .border_color(amber.opacity(0.2))
            .bg(amber.opacity(0.06))
            .text_size(px(12.0))
            .text_color(amber_text.opacity(0.9))
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.0))
            .child(
                div().flex_none().mt(px(2.0)).child(
                    crate::icons::icon(crate::icons::DANGER_TRIANGLE)
                        .size(px(14.0))
                        .text_color(amber_text.opacity(0.9)),
                ),
            )
            .child(div().min_w_0().child(message.into()))
    }
}

impl Status for Theme {}
