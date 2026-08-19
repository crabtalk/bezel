//! Faces, generated rather than chosen from a roster: the shape is a radial
//! profile, so a preset, a name and a seed are the same kind of value.
//!
//! The top row never repeats. Every silhouette is sampled at the same angles,
//! so one face becomes the next by interpolation — there is no morph engine
//! here and no catalogue of transitions, only two poses and a fraction. Click
//! one to stop it on the face it is wearing, click again to let it run.
//!
//! One timer drives every face on the page, which is the shape a chat list
//! wants: motion is a function of `t` rather than state inside each avatar.

use std::time::Duration;

use agent::{
    Face, Pose,
    avatar::{Eyes, Motion, Shape, seed},
};
use gpui::{
    AnyElement, Axis, Context, DragMoveEvent, Empty, Hsla, Render, ScrollHandle, SharedString,
    Subscription, Task, Window, div, prelude::*, px,
};
use theme::Theme;
use ui::{
    focus,
    scroll::{self, TransientState},
    tooltip::Tooltip,
    widgets::{self, ButtonStyle, Buttons, Controls, SliderDrag},
};

use crate::{hint, stack};

const TICK: Duration = Duration::from_millis(40);

/// Seconds a face is held, and seconds it takes to become the next one.
const HOLD: f32 = 1.6;
const MORPH: f32 = 0.9;

/// How many faces cycle side by side, and how far apart they change.
const LANES: usize = 4;
const STAGGER: f32 = 0.7;

/// What the speed slider spans, geometrically — so the middle of the track is
/// 1×, which an arithmetic range would put at 2.1×.
const SLOWEST: f32 = 0.25;
const FASTEST: f32 = 4.0;
/// One press of ← or → on the focused slider.
const NUDGE: f32 = 0.05;

/// What a chat client might actually hand this thing. A seed is bytes, so the
/// row is half names and half everything else a caller calls an identity —
/// and it ends on one person spelled two ways, which is the canonical key
/// doing its job.
///
/// Latin and Cyrillic because that is what the bundled faces carry; a browser
/// has no font book to fall back on, and a row of tofu is a worse claim about
/// a script than not printing it.
const NAMES: &[&str] = &[
    "alain",
    "Grace Hopper",
    "Jürgen Weiß",
    "café-4",
    "Nguyễn",
    "Пётр",
    "@sara",
    "sara@bezel.dev",
    "0x9e3779b9",
    "~/.config",
    "Sara",
    "SARA",
];

/// One cycling face, and whether it is still running.
#[derive(Clone, Copy, Default)]
struct Lane {
    /// When it was stopped, in page time. `None` while it runs.
    held: Option<f32>,
    /// Total time spent stopped, so resuming carries on rather than jumping.
    paused: f32,
}

impl Lane {
    fn at(&self, t: f32) -> f32 {
        self.held.unwrap_or(t) - self.paused
    }

    fn toggle(&mut self, t: f32) {
        match self.held.take() {
            Some(at) => self.paused += t - at,
            None => self.held = Some(t),
        }
    }
}

/// The page: an endless row on top, then the vocabulary it draws from.
pub struct Avatars {
    /// Motion time, which the speed slider never touches — a face breathes at
    /// its own rate however fast the row is changing.
    t: Duration,
    /// Morph time, advanced by the slider's multiple of the tick. Accumulated
    /// rather than derived from `t`, so a speed change carries on from where
    /// the row is instead of jumping to where it would have been.
    cycle: f32,
    /// Where the slider sits, `0..=1`.
    speed: f32,
    /// Which cast of faces the row is drawing. Bumped by the shuffle.
    set: u64,
    lanes: [Lane; LANES],
    slider: gpui::FocusHandle,
    scroll: ScrollHandle,
    bar: TransientState,
    tick: Option<Task<()>>,
    activation: Option<Subscription>,
}

impl Avatars {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            t: Duration::ZERO,
            cycle: 0.0,
            speed: 0.5,
            set: 0,
            lanes: [Lane::default(); LANES],
            slider: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            bar: TransientState::new(),
            tick: None,
            activation: None,
        }
    }

    /// The slider's fraction as a multiple of real time.
    fn rate(&self) -> f32 {
        SLOWEST * (FASTEST / SLOWEST).powf(self.speed)
    }

    fn nudge(&mut self, by: f32, cx: &mut Context<Self>) {
        self.speed = (self.speed + by).clamp(0.0, 1.0);
        cx.notify();
    }

    /// Keep the timer already in flight rather than cancel and reschedule it —
    /// a render between ticks would otherwise push the next frame farther out
    /// every time and stall the animation.
    fn schedule_tick(&mut self, cx: &mut Context<Self>) {
        if self.tick.is_some() {
            return;
        }
        self.tick = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TICK).await;
            let _ = this.update(cx, |this, cx| {
                this.tick = None;
                this.t += TICK;
                this.cycle += TICK.as_secs_f32() * this.rate();
                cx.notify();
            });
        }));
    }

    fn row(children: impl IntoIterator<Item = AnyElement>) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_start()
            .gap(px(18.0))
            .children(children)
    }
}

/// A face under its own name, with the line that draws it on hover.
fn cell(
    id: impl Into<gpui::ElementId>,
    pose: Pose,
    size: f32,
    label: impl Into<SharedString>,
    code: SharedString,
    lit: bool,
    theme: &Theme,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.0))
        .w(px(size + 44.0))
        .child(div().w(px(size)).h(px(size)).child(agent::avatar(pose)))
        .child(
            div()
                .text_size(px(10.0))
                .font_family(theme.font_mono.clone())
                .text_color(if lit { theme.text } else { theme.text_faint })
                .child(label.into()),
        )
        .tooltip(move |window, cx| Tooltip::text(code.clone(), window, cx))
}

/// The foundation's colored tokens, which is where a face's color comes from.
fn palette(theme: &Theme) -> [Hsla; 5] {
    [
        theme.accent,
        theme.success,
        theme.warning,
        theme.danger,
        theme.busy,
    ]
}

/// Face number `n` of `lane`, in cast `set`. Derived rather than stored, so a
/// row can run as long as the window is open without holding anything.
fn nth(set: u64, lane: usize, n: u64) -> u64 {
    let mixed = (lane as u64)
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(n.wrapping_mul(0xd1b5_4a32_d192_ed03))
        .wrapping_add(set.wrapping_mul(0x2545_f491_4f6c_dd1d))
        .wrapping_add(0x2545_f491_4f6c_dd1d);
    let z = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z ^ (z >> 31)
}

/// Where one lane has got to: a held face, or two of them mid-blend. `cycle` is
/// the lane's own clock, which stops when it is clicked; `t` is the page's,
/// which never does — a stopped face still breathes and blinks.
fn cycling(set: u64, lane: usize, cycle: f32, t: f32, motion: Motion, ink: &[Hsla]) -> (Pose, u64) {
    let local = cycle + lane as f32 * STAGGER;
    let n = (local / (HOLD + MORPH)).floor().max(0.0);
    let into = (local - n * (HOLD + MORPH) - HOLD) / MORPH;
    let face = |n: u64| {
        let seed = nth(set, lane, n);
        (
            Face::from(seed)
                .motion(motion)
                .color(ink[(seed >> 33) as usize % ink.len()])
                .pose(t),
            seed,
        )
    };

    let (here, seed) = face(n as u64);
    if into <= 0.0 {
        return (here, seed);
    }
    let k = motion::EASE_IN_OUT.eval(into.min(1.0));
    let (next, next_seed) = face(n as u64 + 1);
    (
        Pose::lerp(&here, &next, k),
        if k > 0.5 { next_seed } else { seed },
    )
}

impl Render for Avatars {
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

        let t = self.t.as_secs_f32();
        let motion = if reduced {
            Motion::STILL
        } else {
            Motion::ALIVE
        };
        let ink = palette(&theme);
        let tint = |i: usize| ink[i % ink.len()];
        let pose = |face: Face| face.motion(motion).pose(t);
        let (lanes, set, cycle, rate) = (self.lanes, self.set, self.cycle, self.rate());

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
                                "The silhouette is a radial profile, not a pick from a list: \
                                 harmonics for the lobed shapes, a polygon blend for the \
                                 cornered ones. Every one is sampled at the same angles, so \
                                 a face becomes the next by interpolation — click one to stop \
                                 it on the seed it is wearing.",
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(14.0))
                                    .child(
                                        div()
                                            .id("avatar-shuffle")
                                            .cursor_pointer()
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.set =
                                                    view.set.wrapping_add(0x9e37_79b9_7f4a_7c15);
                                                cx.notify();
                                            }))
                                            .child(theme.button(
                                                "Shuffle",
                                                ButtonStyle::Ghost,
                                                None,
                                            )),
                                    )
                                    .child(
                                        div().w(px(180.0)).child(
                                            // Tracked rather than `focus::focusable`, which
                                            // rings a focused control with its own outline —
                                            // a box around a track, here. Keys still reach it.
                                            theme
                                                .slider(self.speed)
                                                .key_context(focus::CONTROL_KEY_CONTEXT)
                                                .track_focus(&self.slider)
                                                .id("avatar-speed")
                                            .cursor_pointer()
                                            .on_drag(SliderDrag, |_, _, _, cx| cx.new(|_| Empty))
                                            .on_drag_move(cx.listener(
                                                |view, event: &DragMoveEvent<SliderDrag>, _, cx| {
                                                    view.speed = widgets::axis_fraction(
                                                        event.event.position,
                                                        event.bounds,
                                                        Axis::Horizontal,
                                                        0.0,
                                                    );
                                                    cx.notify();
                                                },
                                            ))
                                            .on_action(cx.listener(
                                                |view, _: &focus::Decrement, _, cx| {
                                                    view.nudge(-NUDGE, cx)
                                                },
                                            ))
                                            .on_action(cx.listener(
                                                |view, _: &focus::Increment, _, cx| {
                                                    view.nudge(NUDGE, cx)
                                                },
                                            )),
                                        ),
                                    )
                                    .child(
                                        div()
                                            .w(px(52.0))
                                            .text_size(px(10.5))
                                            .font_family(theme.font_mono.clone())
                                            .text_color(theme.text_muted)
                                            .child(format!("{rate:.2}×")),
                                    ),
                            )
                            .child(Self::row((0..LANES).map(|lane| {
                                let (pose, seed) =
                                    cycling(set, lane, lanes[lane].at(cycle), t, motion, &ink);
                                cell(
                                    ("lane", lane),
                                    pose,
                                    96.0,
                                    format!("0x{:08x}", seed >> 32),
                                    format!("Face::from(0x{seed:016x})").into(),
                                    lanes[lane].held.is_some(),
                                    &theme,
                                )
                                .cursor_pointer()
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    let cycle = view.cycle;
                                    view.lanes[lane].toggle(cycle);
                                    cx.notify();
                                }))
                                .into_any_element()
                            })))
                            .child(heading(&theme, "Presets"))
                            .child(Self::row(Shape::PRESETS.iter().enumerate().map(
                                |(i, (name, shape))| {
                                    let face = Face::new(*shape).color(tint(i));
                                    cell(
                                        ("preset", i),
                                        pose(face),
                                        64.0,
                                        *name,
                                        format!("Shape::{}", name.to_uppercase()).into(),
                                        false,
                                        &theme,
                                    )
                                    .into_any_element()
                                },
                            )))
                            .child(heading(&theme, "Eyes"))
                            .child(Self::row(Eyes::PRESETS.iter().enumerate().map(
                                |(i, (name, eyes))| {
                                    let face =
                                        Face::new(Shape::BLOB).eyes(*eyes).color(tint(i + 2));
                                    cell(
                                        ("eyes", i),
                                        pose(face),
                                        64.0,
                                        *name,
                                        format!("Eyes::{}", name.to_uppercase()).into(),
                                        false,
                                        &theme,
                                    )
                                    .into_any_element()
                                },
                            )))
                            .child(heading(&theme, "From a name"))
                            .child(Self::row(NAMES.iter().enumerate().map(|(i, name)| {
                                // Tinted by the seed rather than the position,
                                // so two spellings of one person match in
                                // colour as well as in shape.
                                let face = Face::from(*name)
                                    .color(tint((seed(name) >> 33) as usize));
                                cell(
                                    ("name", i),
                                    pose(face),
                                    64.0,
                                    *name,
                                    format!("Face::from({name:?})").into(),
                                    false,
                                    &theme,
                                )
                                .into_any_element()
                            }))),
                    ),
            )
            .child(scroll::transient(
                "avatar-bar",
                &self.scroll,
                &self.bar,
                reduced,
            ))
    }
}

fn heading(theme: &Theme, label: &str) -> AnyElement {
    div()
        .text_size(px(11.0))
        .font_family(theme.font_mono.clone())
        .text_color(theme.text_muted)
        .child(label.to_string())
        .into_any_element()
}
