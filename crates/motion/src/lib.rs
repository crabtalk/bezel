//! Animation kit — a named motion catalog as reusable helpers over gpui
//! [`Animation`]/[`AnimationExt`].
//!
//! Catalog (docs/research/feature-inventory.md §1.12):
//! - `fade-in`   0.5s  cubic-bezier(0.16,1,0.3,1), translateY 4→0 (entrances)
//! - `fade-quick` 0.15s
//! - `menu-in`   0.14s scale 0.96 + translateY −2 (popovers)
//! - `dialog-in` 0.18s scale 0.96→1
//! - `splash-out` 0.5s opacity + translateY −6, 0.15s delay
//! - `pulse` 2.4s staggered cell opacity 0.08→1, scale 0.9→1 (loaders)
//! - `gradient-spin-pulse` 750ms per-cell phase wave (working indicator)
//! - 200ms ease-out width/height transitions (sidebar/panes)
//!
//! Custom easing is a closure over gpui's `Fn(f32) -> f32` easing shape; CSS
//! `cubic-bezier()` is evaluated exactly by [`CubicBezier`].
//!
//! Reduced motion: gpui's `App::reduce_motion` flag is honored *automatically* by
//! every `with_animation` element — oneshot animations snap to their end state,
//! repeating ones to their start state, and no frames are scheduled. The
//! [`set_reduced_motion`]/[`reduced_motion`] wrappers make it a single global
//! switch; pure helpers take the flag explicitly where they run outside elements.
//!
//! translateY is implemented as a relative-position `top` inset: taffy applies
//! relative insets after layout, so — like a CSS transform — siblings never move.
//! gpui has no scale transform for `div`s at the pinned rev (only `svg`
//! transformations), so `menu-in`/`dialog-in` approximate their scale component
//! with fade + translate; see the module report in ARCHITECTURE §4 follow-ups.

use std::{
    cell::RefCell,
    collections::HashMap,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use web_time::Instant;

use gpui::{
    Animation, AnimationElement, App, Context, ElementId, EntityId, Global, Hsla, IntoElement,
    Rgba, SharedString, Styled, Window, px,
};

/// What the catalog's one-shot entrances are built on. Repeats belong on
/// [`pulse_delta`], which leases the view instead of pinning the window.
pub use gpui::AnimationExt;

pub mod phase;

// ---------------------------------------------------------------------------
// The clock — one leased drive for everything that repeats
// ---------------------------------------------------------------------------

/// Redraw rate for the pulse/spinner loaders.
///
/// 2026-08, M-series laptop: one spinner drawn at 120Hz cost 36% of a core —
/// the window redraw, with the animation math itself at 0.3%. 30fps is
/// visually equivalent for these chunky cell waves at a quarter of the draws,
/// and a window with no spinner mounted schedules nothing at all.
const PULSE_FPS: f32 = 30.0;

/// How long a view stays on the tick list after its last spinner paint. One
/// lease outlives a few missed frames; an unmounted spinner stops renewing and
/// the view drops off, letting the clock park.
const PULSE_LEASE: Duration = Duration::from_millis(300);

/// Redraw rate for hover fades. [`HOVER_FADE`] is 150ms, so this paints it in
/// nine steps — the drive it replaces ran at the display's rate, whatever that
/// was.
const HOVER_FPS: f32 = 60.0;

/// Floor on how long the clock sleeps between wake-ups, so a lease asking for
/// an absurd rate cannot turn the loop into a spin.
const MIN_SLEEP: Duration = Duration::from_millis(1);

/// One view's claim on the clock.
struct Lease {
    /// The fastest rate anything on this view has claimed.
    period: Duration,
    /// When this view is next owed a redraw.
    due: Instant,
    /// A notify is out and the render it provoked has not renewed this lease
    /// yet. Read by [`Painter::woken`] — never by the schedule, because a
    /// claim taken from an *event* is renewed by no render at all: a hover
    /// fade would paint one frame and freeze there.
    in_flight: bool,
    /// When the claim lapses if nothing renews it.
    until: Instant,
}

#[derive(Default)]
struct PulseClock {
    /// Set on first use. Everything here reads the executor's clock rather than
    /// `Instant::now()` — one time source for scheduling and for phase, and the
    /// only way a test can advance a second of animation without waiting one.
    epoch: Option<Instant>,
    leases: HashMap<EntityId, Lease>,
    running: bool,
}

impl PulseClock {
    /// When the loop should next wake: the earliest thing owed to anyone, or
    /// the earliest lapse, whichever comes first.
    ///
    /// Every lease answers with a real time. A lease that fell back to its
    /// *lapse* while waiting on a render is what held every drive in the
    /// library to one frame per lease rather than one per period — a spinner
    /// asking for 30fps drew about 5.
    fn next_wake(&self) -> Option<Instant> {
        self.leases
            .values()
            .map(|lease| lease.due.min(lease.until))
            .min()
    }
}

impl Global for PulseClock {}

/// A component's line back to the view that paints it.
///
/// Component state that outlives a render — a hover fade, a gesture — has to
/// name its view, because event-dispatch context cannot resolve one and the
/// only alternative left is refreshing the whole window. Holding a `Painter`
/// is what makes the two sanctioned frame requests reachable, and it is the
/// one surface to review when asking who in the library can ask for a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Painter(EntityId);

impl Painter {
    /// The view this render belongs to. Take it once, where the state is
    /// built, and keep it for as long as the state lives.
    pub fn of<T: 'static>(cx: &Context<T>) -> Self {
        Self(cx.entity_id())
    }

    /// Redraw this view once.
    pub fn notify(self, cx: &mut App) {
        cx.notify(self.0);
    }

    /// Claim `fps` redraws a second, lapsing `until` from now unless something
    /// renews it. One timer serves the whole app: it wakes only when a view is
    /// owed a frame, notifies that view alone, and parks when the last claim
    /// lapses.
    ///
    /// This is the drive for any repeating animation, yours included. gpui's
    /// `with_animation(…).repeat()` asks the *window* for a frame at the
    /// display's rate for as long as it stays mounted, and one such element is
    /// enough to hold the whole window there.
    ///
    /// Renew it from `render` — that is what makes a claim self-cancelling,
    /// since an element that unmounts stops renewing and drops off. The rate is
    /// a per-view minimum of everything claiming it, so a 60fps claim never
    /// drags a 30fps one up with it.
    pub fn lease(self, fps: f32, until: Duration, cx: &mut App) {
        lease(self.0, fps, until, cx);
    }

    /// Whether the clock is what asked for the render running now, rather than
    /// the app. True only in the render a tick provoked: the clock clears what
    /// this view is owed when it notifies, and the render's own [`lease`] is
    /// what puts it back. Read it before renewing.
    pub fn woken(self, cx: &App) -> bool {
        cx.try_global::<PulseClock>()
            .and_then(|clock| clock.leases.get(&self.0))
            .is_some_and(|lease| lease.in_flight)
    }
}

impl From<Painter> for EntityId {
    fn from(painter: Painter) -> Self {
        painter.0
    }
}

impl From<EntityId> for Painter {
    fn from(view: EntityId) -> Self {
        Self(view)
    }
}

fn lease(view: EntityId, fps: f32, until: Duration, cx: &mut App) {
    let now = cx.background_executor().now();
    let period = Duration::from_secs_f32(1.0 / fps.clamp(1.0, 240.0));
    let clock = cx.default_global::<PulseClock>();
    clock
        .leases
        .entry(view)
        .and_modify(|lease| {
            lease.period = lease.period.min(period);
            // The tick already booked the next slot; this render only pulls it
            // earlier if the claim is faster. Re-dating it from *now* would add
            // the frame's own cost to every period — 30fps drew 20.
            lease.due = lease.due.min(now + period);
            lease.in_flight = false;
            lease.until = lease.until.max(now + until);
        })
        .or_insert(Lease {
            period,
            due: now + period,
            in_flight: false,
            until: now + until,
        });
    if clock.running {
        return;
    }
    clock.running = true;
    cx.spawn(async move |cx| {
        loop {
            let sleep = cx.update(|cx| {
                let now = cx.background_executor().now();
                let clock = cx.default_global::<PulseClock>();
                clock
                    .next_wake()
                    .map(|wake| wake.saturating_duration_since(now).max(MIN_SLEEP))
            });
            let Some(sleep) = sleep else { break };
            cx.background_executor().timer(sleep).await;
            let parked = cx.update(|cx| {
                let now = cx.background_executor().now();
                // The clock is the fade store's tick too: it is what advances
                // the frame counter that evicts fades whose elements went away.
                tick_hover_fades();
                let clock = cx.default_global::<PulseClock>();
                clock.leases.retain(|_, lease| lease.until > now);
                if clock.leases.is_empty() {
                    clock.running = false;
                    return true;
                }
                // Owed a frame now. The slot moves on whatever happens next,
                // so the cadence is the period and not the period plus however
                // long the frame took.
                let mut owed = Vec::new();
                for (view, lease) in clock.leases.iter_mut() {
                    if lease.due > now {
                        continue;
                    }
                    lease.due += lease.period;
                    // Frames are costing more than the rate asked for; the next
                    // slot is now rather than a run of slots already missed.
                    if lease.due <= now {
                        lease.due = now + lease.period;
                    }
                    lease.in_flight = true;
                    owed.push(*view);
                }
                for view in owed {
                    cx.notify(view);
                }
                false
            });
            if parked {
                break;
            }
        }
    })
    .detach();
}

/// Current phase `[0,1)` of a repeating spec, plus a [`lease`] that keeps the
/// calling view re-rendering at [`PULSE_FPS`] while its spinner stays mounted.
/// All cells across all views share one epoch, so multi-instance loaders stay
/// phase-locked. Reduced motion returns a static 0 and schedules nothing.
pub fn pulse_delta(spec: &MotionSpec, painter: Painter, cx: &mut App) -> f32 {
    if cx.reduce_motion() {
        return 0.0;
    }
    painter.lease(PULSE_FPS, PULSE_LEASE, cx);
    let now = cx.background_executor().now();
    let clock = cx.default_global::<PulseClock>();
    let epoch = *clock.epoch.get_or_insert(now);
    let period = spec.total().as_secs_f32();
    ((now - epoch).as_secs_f32() / period).fract()
}

// ---------------------------------------------------------------------------
// Cubic bezier
// ---------------------------------------------------------------------------

/// A CSS `cubic-bezier(x1, y1, x2, y2)` timing function (endpoints fixed at
/// (0,0) and (1,1)). Evaluation solves x(t) = input by Newton iteration with a
/// bisection fallback — the standard UnitBezier approach.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl CubicBezier {
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    fn coefficients(a: f32, b: f32) -> (f32, f32, f32) {
        let c = 3.0 * a;
        let bb = 3.0 * (b - a) - c;
        let aa = 1.0 - c - bb;
        (aa, bb, c)
    }

    fn sample_x(&self, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(self.x1, self.x2);
        ((a * t + b) * t + c) * t
    }

    fn sample_y(&self, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(self.y1, self.y2);
        ((a * t + b) * t + c) * t
    }

    fn sample_x_derivative(&self, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(self.x1, self.x2);
        (3.0 * a * t + 2.0 * b) * t + c
    }

    /// Curve parameter `t` for a given progress `x` (both 0..1).
    fn solve_t_for_x(&self, x: f32) -> f32 {
        // Newton–Raphson.
        let mut t = x;
        for _ in 0..8 {
            let err = self.sample_x(t) - x;
            if err.abs() < 1e-6 {
                return t;
            }
            let d = self.sample_x_derivative(t);
            if d.abs() < 1e-6 {
                break;
            }
            t -= err / d;
        }
        // Bisection fallback (x(t) is monotonic for valid CSS beziers).
        let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
        for _ in 0..32 {
            let mid = (lo + hi) / 2.0;
            if self.sample_x(mid) < x {
                lo = mid
            } else {
                hi = mid
            }
        }
        (lo + hi) / 2.0
    }

    /// Eased output for input progress `x ∈ [0,1]` (clamped).
    pub fn eval(&self, x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        // f32 rounding can push sample_y a hair past 1.0 (observed 1.000000119
        // near the end of menu animations); gpui's animation element asserts
        // `delta ∈ [0,1]` and aborts, so clamp the output hard.
        self.sample_y(self.solve_t_for_x(x)).clamp(0.0, 1.0)
    }

    /// This curve as a gpui easing closure.
    pub fn easing(self) -> impl Fn(f32) -> f32 + 'static {
        move |x| self.eval(x)
    }
}

/// The signature entrance curve — CSS `cubic-bezier(0.16, 1, 0.3, 1)`.
pub const EASE_OUT_EXPO: CubicBezier = CubicBezier::new(0.16, 1.0, 0.3, 1.0);
/// CSS `ease-out` — width/height transitions.
pub const EASE_OUT: CubicBezier = CubicBezier::new(0.0, 0.0, 0.58, 1.0);
/// CSS `ease` — quick fades, menu/dialog pops.
pub const EASE: CubicBezier = CubicBezier::new(0.25, 0.1, 0.25, 1.0);
/// Sidebar resort glide — CSS `cubic-bezier(0.22, 1, 0.36, 1)` (used from M3b).
pub const EASE_RESORT: CubicBezier = CubicBezier::new(0.22, 1.0, 0.36, 1.0);
/// CSS `ease-in-out` — the transcript scroll glide (browser smooth-scroll
/// shape: gentle start, cruise, gentle landing).
pub const EASE_IN_OUT: CubicBezier = CubicBezier::new(0.42, 0.0, 0.58, 1.0);

// ---------------------------------------------------------------------------
// Motion specs (the catalog)
// ---------------------------------------------------------------------------

/// One catalog entry: duration + optional delay + curve. The delay is folded into
/// the gpui animation timeline (gpui `Animation` has no native delay): the
/// animation runs for `delay + duration` and [`progress`](Self::progress) holds 0
/// until the delay has elapsed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionSpec {
    pub duration_ms: u64,
    pub delay_ms: u64,
    pub curve: CubicBezier,
}

impl MotionSpec {
    pub const fn new(duration_ms: u64, curve: CubicBezier) -> Self {
        Self {
            duration_ms,
            delay_ms: 0,
            curve,
        }
    }

    pub const fn with_delay(mut self, delay_ms: u64) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    /// Wall-clock span of the whole timeline (delay + duration).
    pub fn total(&self) -> Duration {
        Duration::from_millis(self.delay_ms + self.duration_ms)
    }

    /// Eased progress (0..1) for a raw timeline delta (0..1 across [`total`](Self::total)).
    /// Pure — unit-testable without a window.
    pub fn progress(&self, raw_delta: f32) -> f32 {
        let total = (self.delay_ms + self.duration_ms) as f32;
        if total <= 0.0 || self.duration_ms == 0 {
            return 1.0;
        }
        let t =
            (raw_delta.clamp(0.0, 1.0) * total - self.delay_ms as f32) / self.duration_ms as f32;
        self.curve.eval(t.clamp(0.0, 1.0))
    }

    /// A oneshot gpui [`Animation`] for this spec (delay folded in).
    /// Wall-clock span honors [`speed_scale`] (measurement knob).
    pub fn animation(&self) -> Animation {
        let spec = *self;
        Animation::new(spec.total().mul_f32(speed_scale())).with_easing(move |d| spec.progress(d))
    }
}

/// Entrances: 0.5s expo-out fade + 4px rise.
pub const FADE_IN: MotionSpec = MotionSpec::new(500, EASE_OUT_EXPO);
/// Quick fade: 0.15s.
pub const FADE_QUICK: MotionSpec = MotionSpec::new(150, EASE);
/// Popover-in: 0.14s (scale 0.96 approximated, translateY −2).
pub const MENU_IN: MotionSpec = MotionSpec::new(140, EASE);
/// Popover-out: 0.1s — quicker than the entrance (exits should get out of the
/// way; matches the Radix convention of a shorter close than open).
pub const MENU_OUT: MotionSpec = MotionSpec::new(100, EASE);
/// Dialog-in: 0.18s (scale 0.96→1 approximated).
pub const DIALOG_IN: MotionSpec = MotionSpec::new(180, EASE);
/// Boot splash exit: 0.5s fade + 6px lift after a 0.15s hold.
pub const SPLASH_OUT: MotionSpec = MotionSpec::new(500, EASE).with_delay(150);
/// Sidebar / pane width+height transitions: 200ms ease-out.
pub const RESIZE: MotionSpec = MotionSpec::new(200, EASE_OUT);
/// Terminal tab drag-reorder sliding transforms: 150ms (§1.10).
pub const TAB_SLIDE: MotionSpec = MotionSpec::new(150, EASE_OUT);
/// Diff-pane per-file collapse: 180ms height (§1.11).
pub const COLLAPSE: MotionSpec = MotionSpec::new(180, EASE_OUT);
/// Diff-pane chevron rotate: 200ms (§1.11; approximated as a crossfade — gpui
/// divs have no rotation transform at the pinned rev, same caveat as scale).
pub const CHEVRON: MotionSpec = MotionSpec::new(200, EASE);
/// Rail-tick / scroll-to-row glide: 500ms ease-in-out over the whole distance
/// (Electron parity — the original rail rode the browser's native smooth
/// scroll, a fixed-duration gentle ease, never percent-of-remaining).
pub const SCROLL_GLIDE: MotionSpec = MotionSpec::new(500, EASE_IN_OUT);
/// Tailwind's default transition curve — CSS `cubic-bezier(0.4, 0, 0.2, 1)`
/// (`transition-colors` et al. carry it unless overridden).
pub const EASE_TAILWIND: CubicBezier = CubicBezier::new(0.4, 0.0, 0.2, 1.0);
/// CSS `transition-colors` default: 150ms over [`EASE_TAILWIND`] — the temporal
/// blend every interactive hover wash rides in the original.
pub const HOVER_FADE: MotionSpec = MotionSpec::new(150, EASE_TAILWIND);
/// Pulse loader period: 2.4s.
pub const PULSE: MotionSpec = MotionSpec::new(2400, EASE);
/// Gradient matrix spinner wave period: 750ms.
pub const GRADIENT_SPIN: MotionSpec = MotionSpec::new(750, EASE);
/// Orb cluster breath: 2s, and `EASE_IN_OUT` because a breath has no edges —
/// the two spinners tick, this one swells.
pub const ORB: MotionSpec = MotionSpec::new(phase::ORB_MS, EASE_IN_OUT);

// ---------------------------------------------------------------------------
// Element helpers (paint-layer entrances/exits)
// ---------------------------------------------------------------------------

/// Standard entrance: opacity 0→1 + translateY 4→0 over [`FADE_IN`].
pub fn fade_in<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, FADE_IN.animation(), |el, t| {
        el.relative().opacity(t).top(px(4.0 * (1.0 - t)))
    })
}

/// Quick opacity-only fade over [`FADE_QUICK`].
pub fn fade_quick<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, FADE_QUICK.animation(), |el, t| el.opacity(t))
}

/// Popover entrance: fade + translateY −2→0 over [`MENU_IN`].
/// (the original also scales 0.96→1; divs have no scale transform in gpui — approximated.)
pub fn menu_in<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, MENU_IN.animation(), |el, t| {
        el.relative()
            .opacity(0.3 + 0.7 * t)
            .top(px(-2.0 * (1.0 - t)))
    })
}

/// Popover exit: the reverse of [`menu_in`] — fade to 0 + translateY 0→−2 over
/// [`MENU_OUT`]. Unlike the entrances, the eased progress `t` comes from the
/// caller (computed off `bezel::popover::Popup`'s closing instant at render
/// time): `with_animation`'s element-id-keyed clock replays from 0 on remount
/// (the hover-blend comment's warning), and a replay mid-exit is a full-opacity
/// flash. The wall-clock progress is monotonic by construction; the animation
/// wrapper here only pumps frames for the exit's span, its own delta unused.
pub fn menu_out<E>(id: impl Into<ElementId>, t: f32, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, MENU_OUT.animation(), move |el, _| {
        el.relative().opacity(1.0 - t).top(px(-2.0 * t))
    })
}

/// Dialog entrance over [`DIALOG_IN`] (scale approximated with fade + 2px rise).
pub fn dialog_in<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, DIALOG_IN.animation(), |el, t| {
        el.relative().opacity(t).top(px(2.0 * (1.0 - t)))
    })
}

/// Boot-splash exit: hold 150ms, then fade out + lift 6px over 500ms.
pub fn splash_out<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, SPLASH_OUT.animation(), |el, t| {
        el.opacity(1.0 - t).top(px(-6.0 * t))
    })
}

// ---------------------------------------------------------------------------
// Loader math (pure; rendered by bezel::loaders)
// ---------------------------------------------------------------------------

// The loader constants and math live in `crate::phase` (pure phase
// functions); this crate animates them with gpui.
pub use crate::phase::{
    ORB_BLOOM_RINGS, ORB_RING_DOT, ORB_RING_DOTS, ORB_RING_RADIUS, ORB_SEATS, ORBS,
    PULSE_MIN_OPACITY, PULSE_MIN_SCALE, PULSE_STAGGER, gspin_opacity, orb_bloom_opacity,
    orb_bloom_radius, orb_converge_radius, orb_drift, orb_glow, orb_opacity, orb_ring_seat,
    orb_size, pulse_opacity, pulse_scale, pulse_wave, staggered_phase,
};

/// Gradient-matrix spinner wave: intensity (0..1) of cell `wave_index` out of
/// `wave_count` diagonals, at raw delta `raw_delta` of the 750ms period. The wave
/// front travels across diagonals once per period.
pub fn matrix_wave(raw_delta: f32, wave_index: usize, wave_count: usize) -> f32 {
    let count = wave_count.max(1) as f32;
    pulse_wave(staggered_phase(raw_delta, wave_index, 1.0 / count))
}

/// Linear interpolation (layout tweens).
pub fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

// ---------------------------------------------------------------------------
// Hover color fades (CSS `transition-colors` parity)
// ---------------------------------------------------------------------------
//
// gpui `.hover()` styles snap by construction — the style applies the frame
// the pointer enters. The original CSS puts Tailwind `transition-colors`
// (150ms, cubic-bezier(0.4, 0, 0.2, 1)) on every interactive wash, so hover
// states FADE. This is the manual-drive tween for that (the shell `WidthTween`
// pattern — never `with_animation`, whose element-id-keyed clock replays on
// remount): a per-element-key hover progress, advanced from wall time on each
// evaluation, with the render tail requesting frames while any fade is
// mid-flight.
//
// The store is a main-thread `thread_local` rather than a gpui Global so the
// many free-function element builders (window-control buttons, popover menu
// rows, markdown code blocks) can blend colors without threading `cx` through
// every signature. All access happens on the UI thread (element builders,
// mouse listeners, the render tail).
//
// Staleness: an element that unmounts mid-hover never gets its leave event, so
// entries are stamped with a frame counter on every read and pruned by the
// clock's tick when a full tick passes without a read — a reopened menu never
// inherits a dead entry's wash.

/// One element's hover fade: progress runs `origin → target` over
/// [`HOVER_FADE`], re-anchored at `origin` whenever the pointer flips
/// direction mid-flight so the blend is continuous.
#[derive(Debug, Clone, Copy)]
pub struct FadeEntry {
    origin: f32,
    target: f32,
    started: Instant,
    /// Frame counter at the last read (liveness stamp — see module notes).
    seen: u64,
}

impl FadeEntry {
    fn value(&self, now: Instant, duration: Duration) -> f32 {
        let elapsed = now.saturating_duration_since(self.started);
        if duration.is_zero() || elapsed >= duration {
            return self.target;
        }
        let raw = elapsed.as_secs_f32() / duration.as_secs_f32();
        lerp(self.origin, self.target, HOVER_FADE.curve.eval(raw))
    }

    fn settled(&self, now: Instant, duration: Duration) -> bool {
        self.origin == self.target || now.saturating_duration_since(self.started) >= duration
    }
}

/// Which fade: the view that paints it, and which element inside that view.
///
/// The view is half the identity because the store is one map for the whole
/// app. Keyed on the string alone, two views using `"row-3"` trade each other's
/// wash — which is why the ghost button used to ask callers for a key unique
/// across the entire program.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fade {
    pub painter: Painter,
    pub key: SharedString,
}

impl Fade {
    pub fn new(painter: Painter, key: impl Into<SharedString>) -> Self {
        Self {
            painter,
            key: key.into(),
        }
    }
}

/// Per-fade progress store. Pure core (explicit `now`) — unit-testable; the
/// thread-local wrappers below feed it the clock.
#[derive(Default)]
pub struct HoverFades {
    pub entries: HashMap<Fade, FadeEntry>,
    frame: u64,
}

impl HoverFades {
    pub fn duration() -> Duration {
        HOVER_FADE.total().mul_f32(speed_scale())
    }

    /// Pointer entered (`hovered`) or left the element behind `fade`. Reduced
    /// motion snaps straight to the endpoint.
    pub fn set_at(&mut self, fade: &Fade, hovered: bool, reduced: bool, now: Instant) {
        let target = if hovered { 1.0 } else { 0.0 };
        let duration = Self::duration();
        let current = self
            .entries
            .get(fade)
            .map(|e| e.value(now, duration))
            .unwrap_or(0.0);
        if target == 0.0 && !self.entries.contains_key(fade) {
            return; // never-hovered element reporting a leave — nothing to do
        }
        let origin = if reduced { target } else { current };
        let seen = self.frame;
        self.entries.insert(
            fade.clone(),
            FadeEntry {
                origin,
                target,
                started: now,
                seen,
            },
        );
    }

    /// Hover progress (0..1) for `fade` at `now`; stamps liveness.
    pub fn value_at(&mut self, fade: &Fade, now: Instant) -> f32 {
        let frame = self.frame;
        match self.entries.get_mut(fade) {
            Some(entry) => {
                entry.seen = frame;
                entry.value(now, Self::duration())
            }
            None => 0.0,
        }
    }

    /// Once-per-frame bookkeeping: advance the frame counter, prune entries
    /// that settled back to rest or went a full frame unread (unmounted), and
    /// report whether any fade is still mid-flight (→ keep frames coming).
    pub fn tick_at(&mut self, now: Instant) -> bool {
        self.frame += 1;
        let frame = self.frame;
        let duration = Self::duration();
        let mut active = false;
        self.entries.retain(|_, entry| {
            // Unread through the whole previous frame: the element unmounted
            // (its leave event will never come) — drop the entry.
            if entry.seen + 1 < frame {
                return false;
            }
            let settled = entry.settled(now, duration);
            if !settled {
                active = true;
            }
            // Settled at rest — steady state, indistinguishable from absent.
            !(settled && entry.target == 0.0)
        });
        active
    }
}

thread_local! {
    static HOVER_FADES: RefCell<HoverFades> = RefCell::new(HoverFades::default());
}

/// Hover progress (0..1) for `fade` this frame.
pub fn hover_t(fade: &Fade) -> f32 {
    HOVER_FADES.with(|fades| fades.borrow_mut().value_at(fade, Instant::now()))
}

/// Record a hover flip for `fade` (reduced motion snaps). Prefer
/// [`hover_listener`], which also asks the clock for the frames to paint it.
pub fn set_hover(fade: &Fade, hovered: bool, reduced: bool) {
    HOVER_FADES.with(|fades| {
        fades
            .borrow_mut()
            .set_at(fade, hovered, reduced, Instant::now())
    });
}

/// An `.on_hover` listener driving the fade — pair with [`hover_t`] or
/// [`hover_blend`] reads of the same [`Fade`] in the same element.
///
/// The view comes in on the `Fade` rather than being resolved here: this runs in
/// event-dispatch context, where `Window::current_view()` asserts.
pub fn hover_listener(fade: Fade) -> impl Fn(&bool, &mut Window, &mut App) + 'static {
    move |hovered, _window, cx| {
        set_hover(&fade, *hovered, reduced_motion(cx));
        fade.painter.lease(HOVER_FPS, HoverFades::duration(), cx);
    }
}

/// Once-per-tick bookkeeping for the fade store, driven by the clock.
fn tick_hover_fades() {
    HOVER_FADES.with(|fades| {
        fades.borrow_mut().tick_at(Instant::now());
    });
}

/// Blend two colors by `t` the way the browser transitions them: component
/// interpolation in sRGB with premultiplied alpha — a wash fading in from
/// transparent brightens without passing through grey.
pub fn mix(from: Hsla, to: Hsla, t: f32) -> Hsla {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.0 {
        return from;
    }
    if t >= 1.0 {
        return to;
    }
    let (f, g) = (Rgba::from(from), Rgba::from(to));
    let a = lerp(f.a, g.a, t);
    if a <= f32::EPSILON {
        // Both endpoints (effectively) transparent — carry the target's hue.
        return Hsla::from(Rgba { a: 0.0, ..g });
    }
    Hsla::from(Rgba {
        r: lerp(f.r * f.a, g.r * g.a, t) / a,
        g: lerp(f.g * f.a, g.g * g.a, t) / a,
        b: lerp(f.b * f.a, g.b * g.a, t) / a,
        a,
    })
}

/// The standard hover blend: rest → hover color at this fade's progress.
pub fn hover_blend(fade: &Fade, rest: Hsla, hover: Hsla) -> Hsla {
    mix(rest, hover, hover_t(fade))
}

// ---------------------------------------------------------------------------
// Speed and reduced motion
// ---------------------------------------------------------------------------

/// Process-wide motion speed, as f32 bits: every catalog timeline is multiplied
/// by it.
///
/// An atomic mirror rather than a gpui global, for exactly the reason
/// `theme::current_appearance` is one: the timelines are read from free
/// functions deep inside element builders — [`HoverFades::duration`], the
/// popover exit clock — that have no `cx` in scope. Speed is genuinely
/// process-wide, one setting for every window, so a single mirror is sound.
static SPEED: AtomicU32 = AtomicU32::new(1.0f32.to_bits());

/// How far every timeline in the catalog is stretched. `1.0` is the designed
/// speed.
pub fn speed_scale() -> f32 {
    f32::from_bits(SPEED.load(Ordering::Relaxed))
}

/// Stretch every catalog timeline by `scale` — `10.0` slows the 200ms pane
/// tweens to 2s, so a screenshot burst can sample the geometry frame by frame.
///
/// Configuration in code, like the rest of bezel: an app that wants this on a
/// setting, or hanging off its own theme, calls this from wherever that lives.
/// It used to read a `BEZEL_MOTION_SCALE` environment variable, which meant the
/// one knob in the library that no app could reach.
///
/// Clamped to `0.01..=100.0`; a non-finite `scale` resets to `1.0` rather than
/// poisoning every duration with a NaN.
pub fn set_speed(scale: f32) {
    let scale = if scale.is_finite() {
        scale.clamp(0.01, 100.0)
    } else {
        1.0
    };
    SPEED.store(scale.to_bits(), Ordering::Relaxed);
}

/// [`SPEED`] is process-wide, so a test that moves it must hold this lock and
/// restore `1.0` before letting go — every fade and exit timing asserted
/// anywhere is measured in it. The same arrangement as
/// `theme::lock_appearance`, and public for the same reason: such tests
/// exist in other crates too. Not part of the API.
#[doc(hidden)]
pub fn lock_speed() -> std::sync::MutexGuard<'static, ()> {
    static SPEED_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    SPEED_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Global reduced-motion flag. gpui snaps every `with_animation` element when
/// set (end state for oneshots, rest state for loops) and schedules no frames.
pub fn set_reduced_motion(cx: &mut App, reduced: bool) {
    cx.set_reduce_motion(reduced);
}

/// Read the global reduced-motion flag.
pub fn reduced_motion(cx: &App) -> bool {
    cx.reduce_motion()
}
