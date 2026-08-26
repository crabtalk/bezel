//! What the library costs a window that is sitting still, against what the
//! obvious way of writing the same thing costs it.
//!
//! Two numbers say it. `simulate_next_frame` returns how many per-frame
//! callbacks were waiting — anything above zero is an element asking to be
//! redrawn at the display's rate, for as long as it stays mounted, and it asks
//! on behalf of the whole window. The render count is what that costs, since a
//! redraw rebuilds the element tree, re-runs layout and re-shapes text.
//!
//! [`Drive`] is the same animation written three ways, which is what makes the
//! comparison fair: gpui's default, gpui's own throttle, and bezel's shared
//! clock.
//!
//! The test clock is why these are assertions rather than a stopwatch:
//! `advance_clock` moves the executor's timers without moving the wall clock,
//! so a second of spinner passes in microseconds. It also means
//! `PulseClock`'s leases — held against `Instant::now()` — never expire here,
//! which is why parking is watched on the gallery's overlay instead.

use std::{cell::RefCell, rc::Rc, time::Duration};

use gpui::{
    Animation, AnimationExt, AnyElement, Context, IntoElement, ParentElement, Render, Styled,
    TestAppContext, Window, WindowHandle, div, px, size,
};
use theme::{Appearance, Theme};
use ui::loaders;

/// One second of animation, stepped finely enough that a 33ms timer re-arms
/// inside it — and not in tick-sized steps, which would only prove a loader
/// renders when told to.
const SECOND: Duration = Duration::from_secs(1);
const STEP: Duration = Duration::from_millis(10);

/// The rate the drives used to run at, and the bar the throttled ones stay
/// under. A display is 60Hz or more; the machine the 36% CPU was measured on
/// runs at this.
const DISPLAY_RATE: usize = 120;

/// What the throttled drives are asked for, and what bezel's clock already
/// runs at.
const THROTTLE_FPS: f32 = 30.0;

/// The period of the animation under test — long enough that a repeat is not
/// what any of these are measuring.
const PERIOD: Duration = Duration::from_secs(2);

/// How the same animation gets its frames.
#[derive(Clone, Copy)]
enum Drive {
    /// Nothing animated at all.
    Still,
    /// `with_animation(…).repeat()` — what gpui's docs and examples reach for.
    DisplayRate,
    /// gpui's own throttle: a timer per animated element.
    Throttled,
    /// bezel's `PulseClock`: one timer per app, leased per view.
    SharedClock,
}

struct Counted {
    renders: Rc<RefCell<usize>>,
    drive: Drive,
}

impl Render for Counted {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        *self.renders.borrow_mut() += 1;
        let body: AnyElement = match self.drive {
            Drive::Still => div().into_any_element(),
            Drive::DisplayRate => div()
                .with_animation(
                    "display-rate",
                    Animation::new(PERIOD).repeat(),
                    |el, delta| el.opacity(delta),
                )
                .into_any_element(),
            Drive::Throttled => div()
                .with_animation(
                    "throttled",
                    Animation::new(PERIOD).repeat().with_max_fps(THROTTLE_FPS),
                    |el, delta| el.opacity(delta),
                )
                .into_any_element(),
            Drive::SharedClock => {
                let theme = Theme::of(cx).clone();
                let view = cx.entity_id();
                loaders::pulse_loader("pulse", &theme, 8.0, view, cx).into_any_element()
            }
        };
        div().size_full().child(body)
    }
}

fn open(cx: &mut TestAppContext, drive: Drive) -> (Rc<RefCell<usize>>, WindowHandle<Counted>) {
    cx.update(|cx| Theme::install_custom(Theme::for_appearance(Appearance::Dark), cx));
    let renders = Rc::new(RefCell::new(0));
    let window = cx.open_window(size(px(200.), px(200.)), {
        let renders = renders.clone();
        move |_, _| Counted { renders, drive }
    });
    cx.run_until_parked();
    (renders, window)
}

/// Per-frame callbacks pending right now — how many elements are asking the
/// window for another frame at the display's pace.
fn pending_frames(window: &WindowHandle<Counted>, cx: &mut TestAppContext) -> usize {
    let pending = window
        .update(cx, |_, window, cx| window.simulate_next_frame(cx))
        .unwrap();
    cx.run_until_parked();
    pending
}

fn advance_a_second(cx: &mut TestAppContext) {
    for _ in 0..(SECOND.as_millis() / STEP.as_millis()) {
        cx.executor().advance_clock(STEP);
        cx.run_until_parked();
    }
}

#[gpui::test]
fn a_window_with_nothing_moving_draws_nothing_more(cx: &mut TestAppContext) {
    let (renders, window) = open(cx, Drive::Still);
    let settled = *renders.borrow();

    assert_eq!(
        pending_frames(&window, cx),
        0,
        "a still window asked for another frame"
    );
    advance_a_second(cx);
    assert_eq!(
        *renders.borrow(),
        settled,
        "a still window redrew itself over an idle second"
    );
}

/// The case the library exists to avoid. It never stops asking for the next
/// frame, and the ask is the *window's*, not this element's — one mounted
/// spinner is what pinned a whole window at 120Hz for 36% of a core.
#[gpui::test]
fn gpuis_default_drive_never_stops_asking_for_frames(cx: &mut TestAppContext) {
    let (renders, window) = open(cx, Drive::DisplayRate);
    let settled = *renders.borrow();

    // Not a tail that runs down: every frame it is handed, it asks for another.
    for frame in 1..=10 {
        assert_eq!(
            pending_frames(&window, cx),
            1,
            "stopped asking for frames at frame {frame}, which this drive never does"
        );
    }
    assert_eq!(
        *renders.borrow() - settled,
        10,
        "every frame asked for was a full redraw"
    );
}

/// gpui grew its own answer to the above — `with_max_fps` swaps the per-frame
/// callback for a timer and notifies just the one view. Raw gpui is no longer
/// stuck at display rate; it is only *defaulted* there.
#[gpui::test]
fn gpuis_throttle_leaves_no_per_frame_callback(cx: &mut TestAppContext) {
    let (renders, window) = open(cx, Drive::Throttled);
    let settled = *renders.borrow();

    assert_eq!(
        pending_frames(&window, cx),
        0,
        "the throttled drive asked for a frame at display rate"
    );
    advance_a_second(cx);
    let drawn = *renders.borrow() - settled;
    assert!(
        drawn > 0 && drawn < DISPLAY_RATE / 2,
        "throttled to {THROTTLE_FPS}fps but drew {drawn} times in a second"
    );
}

#[gpui::test]
fn a_mounted_loader_never_drives_the_window_at_display_rate(cx: &mut TestAppContext) {
    let (renders, window) = open(cx, Drive::SharedClock);
    let settled = *renders.borrow();

    assert_eq!(
        pending_frames(&window, cx),
        0,
        "the loader asked for a frame at display rate"
    );

    advance_a_second(cx);
    let drawn = *renders.borrow() - settled;
    assert!(
        drawn > 0,
        "the loader stopped animating: it drew {drawn} times in a second"
    );
    assert!(
        drawn < DISPLAY_RATE / 2,
        "the loader drew {drawn} times in a second, near the display's {DISPLAY_RATE}"
    );
}
