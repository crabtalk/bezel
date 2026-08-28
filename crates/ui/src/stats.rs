//! [`Stats`] — the meter: how many frames this window is drawing, and what the
//! process and the GPU cost, while you watch it.
//!
//! A window at rest reads `0`. The count comes from this view's own renders,
//! which is the same number: gpui re-renders every uncached view once per
//! window draw. The one render it does not count is the one its own tick
//! provoked, and [`Painter::woken`] is what says which that was — the clock
//! knows, so nothing here has to infer it from a stopwatch. Those two draws a
//! second are the meter's own cost, and the CPU figure includes them.
//!
//! Placement is the caller's, as it is for [`crate::control_bar`]:
//!
//! ```ignore
//! let meter = cx.new(Stats::new);
//! div().relative().size_full()
//!     .child(page)
//!     .child(div().absolute().top(px(16.0)).right(px(16.0)).child(meter.clone()))
//! ```
//!
//! [`crate::floating::panel`] is what makes one draggable.

use std::time::Duration;

use gpui::{
    Context, IntoElement, ParentElement as _, Render, SharedString, Styled as _, Window, div, px,
};
use motion::Painter;
use theme::Theme;
use web_time::Instant;

use crate::{
    material::{self, Glass as _},
    popover,
};

/// How often the meter refreshes once nothing else is drawing, and so the span
/// each reading is measured over.
const TICK: Duration = Duration::from_millis(500);

/// How long the claim on the clock outlives the render that took it.
const LEASE: Duration = Duration::from_secs(1);

/// The box's width. Public because a host placing the meter by its trailing
/// edge has to know how wide it is.
pub const WIDTH: f32 = 148.0;

/// Width of the value column, so a digit arriving or leaving never reflows the
/// row it is in.
const VALUE_WIDTH: f32 = 64.0;

/// The frame and CPU meter. One per window — two mounted meters each count the
/// other's frames, and neither reads zero again.
pub struct Stats {
    painter: Painter,
    /// Draws this bucket that the meter did not ask for.
    frames: u32,
    since: Instant,
    /// Process CPU time at [`Self::since`].
    cpu_since: Option<Duration>,
    /// GPU time spent on this window's frames at [`Self::since`].
    gpu_since: Option<Duration>,
    /// Held between recomputes, so the digits stand still long enough to read.
    reading: Reading,
}

#[derive(Clone, Copy, Default)]
struct Reading {
    fps: f32,
    /// Percent of one core, the figure Activity Monitor prints.
    cpu: Option<f32>,
    /// Percent of wall time the GPU was busy on this window's frames. `None`
    /// off Metal, where nothing measures it.
    gpu: Option<f32>,
}

impl Stats {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            painter: Painter::of(cx),
            frames: 0,
            since: Instant::now(),
            cpu_since: cpu_time(),
            gpu_since: None,
            reading: Reading::default(),
        }
    }
}

impl Render for Stats {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.painter.woken(cx) {
            self.frames += 1;
        }

        let now = Instant::now();
        let elapsed = now.duration_since(self.since);
        if elapsed >= TICK {
            let share = |then: Duration, now: Duration| {
                now.saturating_sub(then).as_secs_f32() / elapsed.as_secs_f32() * 100.0
            };
            let (cpu, gpu) = (cpu_time(), window.gpu_time());
            self.reading = Reading {
                fps: self.frames as f32 / elapsed.as_secs_f32(),
                cpu: self.cpu_since.zip(cpu).map(|(then, cpu)| share(then, cpu)),
                gpu: self.gpu_since.zip(gpu).map(|(then, gpu)| share(then, gpu)),
            };
            self.frames = 0;
            self.since = now;
            self.cpu_since = cpu;
            self.gpu_since = gpu;
        }

        self.painter.lease(1.0 / TICK.as_secs_f32(), LEASE, cx);

        let theme = Theme::of(cx).clone();
        let reading = self.reading;
        let card = popover::popover_card(&theme)
            .w(px(WIDTH))
            .p(px(10.0))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(row(&theme, "FPS", format!("{:.0}", reading.fps)))
            .child(row(
                &theme,
                "CPU",
                reading
                    .cpu
                    .map_or_else(|| "—".to_string(), |cpu| format!("{cpu:.1}%")),
            ))
            .child(row(
                &theme,
                "GPU",
                reading
                    .gpu
                    .map_or_else(|| "—".to_string(), |gpu| format!("{gpu:.1}%")),
            ));

        card.material(material::PANEL_BLUR)
    }
}

fn row(theme: &Theme, label: &'static str, value: String) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(10.0))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme.text_faint)
                .child(label),
        )
        .child(
            div()
                .w(px(VALUE_WIDTH))
                .text_right()
                .font_family(theme.font_mono.clone())
                .text_size(px(13.0))
                .text_color(theme.text)
                .child(SharedString::from(value)),
        )
}

/// Process CPU time — user plus system, every thread. `None` where the platform
/// has no such call.
#[cfg(unix)]
fn cpu_time() -> Option<Duration> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: getrusage fills the struct it is handed, and only on success.
    let usage = unsafe {
        if libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) != 0 {
            return None;
        }
        usage.assume_init()
    };
    let spent = |time: libc::timeval| {
        Duration::from_secs(time.tv_sec as u64) + Duration::from_micros(time.tv_usec as u64)
    };
    Some(spent(usage.ru_utime) + spent(usage.ru_stime))
}

#[cfg(not(unix))]
fn cpu_time() -> Option<Duration> {
    None
}
