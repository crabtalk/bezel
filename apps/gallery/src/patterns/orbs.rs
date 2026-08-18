//! The thinking screen — what an agent surface shows while the model works.
//!
//! The orbs themselves are `bezel_ui::orbs`, a component; this page is the
//! pattern: one live [`Orb`] entity (the builder API a real app reaches for)
//! above the twelve-state catalog, painted by [`orb_element`] on this page's
//! own clock. One timer drives every cell — the shape to copy when your host
//! already ticks.
//!
//! The engine takes unbounded time: its modes mix incommensurate frequencies,
//! so a folded clock (gpui's `with_animation`) jumps visibly at every wrap.
//! That is why the component is an entity and this page owns an `Instant`.

use std::time::{Duration, Instant};

use bezel_theme::Theme;
use bezel_ui::{
    focus,
    orbs::{Orb, OrbSize, OrbState, orb_element},
    widgets::Controls,
};
use gpui::{Context, Entity, Render, SharedString, Window, div, prelude::*, px};

use crate::{hint, pressable, stack};

/// The page: size selection, one featured orb, twelve states.
pub struct Orbs {
    started: Instant,
    size: OrbSize,
    featured: Entity<Orb>,
    segments: [gpui::FocusHandle; 4],
}

impl Orbs {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
                let Ok(()) = this.update(cx, |_, cx| cx.notify()) else {
                    return;
                };
            }
        })
        .detach();
        Self {
            started: Instant::now(),
            size: OrbSize::Avatar,
            featured: cx.new(|_| Orb::new().state(OrbState::Searching).size(OrbSize::Avatar)),
            segments: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
        }
    }

    fn cell(state: OrbState, size: OrbSize, t: f32, theme: &Theme) -> gpui::Div {
        div()
            .w(px(size.pixels() + 40.0))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.0))
            .child(orb_element(state, size, t))
            .child(
                div()
                    .text_size(px(10.5))
                    .font_family(theme.font_mono.clone())
                    .text_color(theme.text_faint)
                    .child(state.label()),
            )
    }
}

impl Render for Orbs {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let t = if cx.reduce_motion() {
            0.6
        } else {
            self.started.elapsed().as_secs_f32()
        };
        let size = self.size;

        stack()
            .child(hint(
                &theme,
                "A port of gpui-thinking-orbs: twelve hand-tuned states, four \
                 sizes, monochrome ink over any substrate. The featured orb is \
                 the builder entity; the grid below is `orb_element` driven by \
                 this page's timer.",
            ))
            .child(
                theme
                    .toggle_group()
                    .children(OrbSize::ALL_SIZES.iter().copied().enumerate().map(
                        |(index, size)| {
                            pressable(
                                focus::focusable(
                                    &theme,
                                    &self.segments[index],
                                    theme.toggle_group_item(size.label(), self.size == size),
                                ),
                                SharedString::from(format!("orb-size-{index}")),
                                cx,
                                move |view, cx| {
                                    view.size = size;
                                    view.featured.update(cx, |orb, cx| orb.set_size(size, cx));
                                    cx.notify();
                                },
                            )
                            .into_any_element()
                        },
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(16.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_family(theme.font_mono.clone())
                            .text_color(theme.text_muted)
                            .child("Orb::new().state(..).size(..)"),
                    )
                    .child(self.featured.clone()),
            )
            .child(
                div().flex().flex_row().flex_wrap().gap(px(20.0)).children(
                    OrbState::ALL_STATES
                        .iter()
                        .map(|state| Self::cell(*state, size, t, &theme)),
                ),
            )
    }
}
