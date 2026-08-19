//! The thinking screen — what an agent surface shows while the model works.
//!
//! The orbs themselves are `agent::orbs`, a component; this page is the
//! pattern: the twelve-state catalog, painted by [`orb_element`] on this page's
//! own clock. One timer drives every cell — the shape to copy when your host
//! already ticks.
//!
//! That timer is scheduled from `render`, never from a loop of its own: a page
//! that is scrolled away, reduced-motion, or in a background window simply
//! stops rendering, and the animation stops with it.
//!
//! The clock counts ticks rather than wall time, so a page that comes back
//! after ten minutes in the background resumes where it stopped instead of
//! jumping ten minutes forward.
//!
//! The engine takes unbounded time: its modes mix incommensurate frequencies,
//! so a folded clock (gpui's `with_animation`) jumps visibly at every wrap.
//! That is why this page owns its own clock.

use std::{cell::RefCell, rc::Rc, time::Duration};

use agent::orbs::{OrbSize, OrbState, engine::Frame, orb_element};
use gpui::{
    Context, Render, ScrollHandle, SharedString, Subscription, Task, Window, div, prelude::*, px,
};
use theme::Theme;
use ui::{
    focus,
    scroll::{self, TransientState},
    widgets::Controls,
};

use crate::{hint, pressable, stack};

const TICK: Duration = Duration::from_millis(40);

/// The page: size selection, twelve states.
pub struct Orbs {
    /// Animation time, advanced one [`TICK`] per tick that actually fired.
    t: Duration,
    size: OrbSize,
    segments: [gpui::FocusHandle; 4],
    scroll: ScrollHandle,
    bar: TransientState,
    /// One geometry buffer per cell, reused for the life of the page.
    frames: Vec<Rc<RefCell<Frame>>>,
    /// Pending redraw. At most one in flight; dropping it stops the grid.
    tick: Option<Task<()>>,
    /// Registered lazily on first render, since it needs a `Window`.
    activation: Option<Subscription>,
}

impl Orbs {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            t: Duration::ZERO,
            size: OrbSize::Avatar,
            segments: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            scroll: ScrollHandle::new(),
            bar: TransientState::new(),
            frames: OrbState::ALL_STATES
                .iter()
                .map(|_| Rc::new(RefCell::new(Frame::new())))
                .collect(),
            tick: None,
            activation: None,
        }
    }

    /// Keep the timer already in flight rather than cancel and reschedule it —
    /// a render between ticks (a click, a scroll) would otherwise push the next
    /// frame farther out every time and stall the animation.
    fn schedule_tick(&mut self, cx: &mut Context<Self>) {
        if self.tick.is_some() {
            return;
        }
        self.tick = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TICK).await;
            let _ = this.update(cx, |this, cx| {
                this.tick = None;
                this.t += TICK;
                cx.notify();
            });
        }));
    }

    fn cell(
        state: OrbState,
        size: OrbSize,
        t: f32,
        frame: &Rc<RefCell<Frame>>,
        theme: &Theme,
    ) -> gpui::Div {
        div()
            .w(px(size.pixels() + 40.0))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.0))
            .child(orb_element(state, size, t, frame))
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A backgrounded page stops rendering, so it needs a nudge to pick the
        // clock back up when the window returns.
        if self.activation.is_none() {
            self.activation = Some(cx.observe_window_activation(window, |_, _, cx| cx.notify()));
        }

        let theme = Theme::of(cx).clone();
        let reduced = cx.reduce_motion();
        if reduced || !window.is_window_active() {
            self.tick = None;
        } else {
            self.schedule_tick(cx);
        }

        let t = if reduced { 0.6 } else { self.t.as_secs_f32() };
        let size = self.size;

        div()
            .relative()
            .size_full()
            .child(
                div()
                    .id("orbs-page")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(
                        stack()
                            .child(hint(
                                &theme,
                                "A port of gpui-thinking-orbs: twelve hand-tuned states, four \
                 sizes, monochrome ink over any substrate. The grid is \
                 `orb_element` driven by this page's timer.",
                            ))
                            .child(theme.toggle_group().children(
                                OrbSize::ALL_SIZES.iter().copied().enumerate().map(
                                    |(index, size)| {
                                        pressable(
                                            focus::focusable(
                                                &theme,
                                                &self.segments[index],
                                                theme.toggle_group_item(
                                                    size.label(),
                                                    self.size == size,
                                                ),
                                            ),
                                            SharedString::from(format!("orb-size-{index}")),
                                            cx,
                                            move |view, cx| {
                                                view.size = size;
                                                cx.notify();
                                            },
                                        )
                                        .into_any_element()
                                    },
                                ),
                            ))
                            .child(div().flex().flex_row().flex_wrap().gap(px(20.0)).children(
                                OrbState::ALL_STATES.iter().zip(&self.frames).map(
                                    |(state, frame)| Self::cell(*state, size, t, frame, &theme),
                                ),
                            )),
                    ),
            )
            .child(scroll::transient(
                "orbs-bar",
                &self.scroll,
                &self.bar,
                cx.reduce_motion(),
            ))
    }
}
