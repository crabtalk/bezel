//! Deterministic identity — the same name renders the same face at any size,
//! and two names never share a face.
//!
//! The component is `agent::avatar(name)`, a plain element the caller sizes;
//! this page is the pattern: a name grid over a static page. No state, no
//! timer — an avatar has no clock, and the page that shows one should not
//! tick either.

use agent::avatar;
use gpui::{Context, Render, ScrollHandle, Styled, Window, div, prelude::*, px};
use theme::Theme;
use ui::scroll::{self, TransientState};

use crate::{hint, stack};

/// The names a chat client might actually show.
const NAMES: &[&str] = &[
    "alain",
    "Sara",
    "🦊",
    "café-4",
    "Grace Hopper",
    "Team Rocket 3",
    "Ada",
    "Claude",
];

const SIZES: &[f32] = &[48.0, 64.0, 96.0];

/// The page: one row per name, three sizes per row, scrolling when the pane
/// is shorter than the grid.
#[derive(Default)]
pub struct Avatars {
    scroll: ScrollHandle,
    bar: TransientState,
}

impl Render for Avatars {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        div()
            .relative()
            .size_full()
            .child(
                div()
                    .id("avatar-page")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(
                        stack()
                            .child(hint(
                                &theme,
                                "A port of blobatar: the name is the seed, so the face is \
                         fixed — same person, same avatar, every surface. The palette \
                         comes from the name too, never from the theme.",
                            ))
                            .children(NAMES.iter().map(|name| {
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(20.0))
                                    .child(
                                        div()
                                            .w(px(110.0))
                                            .text_size(px(10.5))
                                            .font_family(theme.font_mono.clone())
                                            .text_color(theme.text_muted)
                                            .child(*name),
                                    )
                                    .children(SIZES.iter().map(|size| {
                                        div()
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(
                                                div()
                                                    .w(px(*size))
                                                    .h(px(*size))
                                                    .child(avatar::avatar(name)),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(9.5))
                                                    .font_family(theme.font_mono.clone())
                                                    .text_color(theme.text_faint)
                                                    .child(format!("{size:.0}px")),
                                            )
                                    }))
                            })),
                    ),
            )
            .child(scroll::transient(
                "avatar-bar",
                &self.scroll,
                &self.bar,
                cx.reduce_motion(),
            ))
    }
}
