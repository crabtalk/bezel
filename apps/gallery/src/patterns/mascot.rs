//! The face as a list glyph: the seed a chat header draws smooth, sampled onto
//! an eight-cell grid for a rail row.
//!
//! The rail is the honest test. A mascot has to read at twelve pixels, and it
//! has to survive being the dim half of a list — which is why the eyes are
//! holes rather than ink: they dim with the body instead of against it.
//!
//! Frames come from the shared clock, claimed from `render` so the claim
//! lapses on its own when the page goes away. Page time is read off that same
//! clock rather than accumulated, so a dropped frame slows nothing.

use std::time::Duration;

use agent::{Face, avatar::Motion};
use gpui::{
    Context, Render, ScrollHandle, SharedString, Subscription, Window, div, prelude::*, px,
};
use motion::Painter;
use theme::Theme;
use web_time::Instant;

use crate::{hint, stack};

/// Redraws a second, matching the rate the sibling pattern pages tick at.
const FPS: f32 = 25.0;

/// How long a claim outlives its last renewal, so a missed frame or two cannot
/// park the clock in the middle of a blink.
const LEASE: Duration = Duration::from_millis(300);

/// Cells a side, drawn big enough here that you can count them.
const SPRITE: f32 = 44.0;

/// What the glyph is actually for.
const ROW: f32 = 13.0;

/// What a rail holds. A seed is bytes, so half of these are not names.
const PROJECTS: &[&str] = &[
    "bezel",
    "gpui",
    "agent-sdk",
    "~/code/notes",
    "src-tauri",
    "0x9e3779b9",
    "Grace Hopper",
];

#[derive(Default)]
pub struct Mascots {
    /// The page's zero, taken from the clock the lease schedules on.
    start: Option<Instant>,
    selected: usize,
    scroll: ScrollHandle,
    activation: Option<Subscription>,
}

impl Render for Mascots {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A backgrounded page stops rendering, so it needs a nudge to pick the
        // clock back up when the window returns.
        if self.activation.is_none() {
            self.activation = Some(cx.observe_window_activation(window, |_, _, cx| cx.notify()));
        }

        let theme = Theme::of(cx).clone();
        let reduced = cx.reduce_motion();
        let now = cx.background_executor().now();
        let start = *self.start.get_or_insert(now);
        if !reduced && window.is_window_active() {
            Painter::of(cx).lease(FPS, LEASE, cx);
        }

        let t = (now - start).as_secs_f32();
        let motion = if reduced {
            Motion::STILL
        } else {
            Motion::ALIVE
        };
        let face = |name: &str| Face::from(name).motion(motion);
        let selected = self.selected;

        let rail = PROJECTS.iter().enumerate().map(|(i, name)| {
            let lit = i == selected;
            div()
                .id(SharedString::from(format!("rail-{i}")))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(9.0))
                .py(px(5.0))
                .px(px(8.0))
                .rounded(px(6.0))
                .cursor_pointer()
                .when(lit, |row| row.bg(theme.surface_raised))
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.selected = i;
                    cx.notify();
                }))
                .child(div().w(px(ROW)).h(px(ROW)).child(agent::mascot(
                    &face(name).color(if lit { theme.accent } else { theme.text_faint }),
                    t,
                )))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(if lit { theme.text } else { theme.text_faint })
                        .child(SharedString::from(name.to_string())),
                )
                .into_any_element()
        });

        let pairs = PROJECTS.iter().take(5).map(|name| {
            let one = face(name);
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_end()
                        .gap(px(10.0))
                        .child(
                            div()
                                .w(px(SPRITE))
                                .h(px(SPRITE))
                                .child(agent::avatar(one.pose(t))),
                        )
                        .child(
                            div()
                                .w(px(SPRITE))
                                .h(px(SPRITE))
                                .child(agent::mascot(&one, t)),
                        )
                        .child(div().w(px(ROW)).h(px(ROW)).child(agent::mascot(&one, t))),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(theme.font_mono.clone())
                        .text_color(theme.text_faint)
                        .child(SharedString::from(name.to_string())),
                )
                .into_any_element()
        });

        div()
            .id("mascot-page")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(
                stack()
                    .child(hint(
                        &theme,
                        "The same seed the blob avatar draws, sampled onto eight cells. \
                         Nothing is picked from a roster: the silhouette is still the radial \
                         profile, so a name that has never been seen has a mascot already. \
                         Eyes are holes rather than ink, which is what lets a row dim whole.",
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(1.0))
                            .w(px(210.0))
                            .p(px(6.0))
                            .rounded(px(9.0))
                            .border_1()
                            .border_color(theme.border)
                            .children(rail),
                    )
                    .child(hint(
                        &theme,
                        "One identity at two resolutions — smooth, as cells, and at the size \
                         a rail actually draws it.",
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .items_start()
                            .gap(px(22.0))
                            .children(pairs),
                    ),
            )
    }
}
