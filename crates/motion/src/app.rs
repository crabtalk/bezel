//! [`AppExt`] — the app-level motion settings, reached on the `App` itself the
//! way component groups are reached on the theme.
//!
//! What belongs here is a setting the `App` holds. [`crate::set_speed`] does
//! not: the catalog's timelines are read from free functions deep inside
//! element builders that have no `cx` to hand, which is why speed is a
//! process-wide mirror instead.

use gpui::{App, Global};

/// Whether animation stops while the app is not frontmost. Absent means on —
/// a global nobody installed is the default, not the opposite of it.
struct PauseWhenInactive(bool);

impl Global for PauseWhenInactive {}

/// The motion settings an app carries.
///
/// ```ignore
/// use motion::AppExt as _;
///
/// cx.set_pause_when_inactive(false);   // a HUD that must keep moving unfocused
/// ```
pub trait AppExt {
    /// gpui snaps every `with_animation` element when this is set — end state
    /// for oneshots, rest state for loops — and schedules no frames.
    fn reduced_motion(&self) -> bool;

    fn set_reduced_motion(&mut self, reduced: bool);

    /// Whether animation stops while the app is not frontmost. On by default.
    fn pause_when_inactive(&self) -> bool;

    /// An app in the background that keeps animating is spending a core on
    /// frames nobody is looking at. Nothing below this stops on its own: one
    /// spinner in a backgrounded window held 30fps and 22% of a core
    /// indefinitely (2026-08, debug build, M-series laptop), against 2% with
    /// this on. Turn it off for a window that must keep moving while something
    /// else has focus — a side panel, a floating HUD.
    ///
    /// The claim is refused rather than cancelled, so nothing has to resume it:
    /// gpui refreshes a window when it becomes active, and the render that
    /// follows takes the claim again.
    fn set_pause_when_inactive(&mut self, pause: bool);
}

impl AppExt for App {
    fn reduced_motion(&self) -> bool {
        self.reduce_motion()
    }

    fn set_reduced_motion(&mut self, reduced: bool) {
        self.set_reduce_motion(reduced);
    }

    fn pause_when_inactive(&self) -> bool {
        self.try_global::<PauseWhenInactive>()
            .is_none_or(|pause| pause.0)
    }

    fn set_pause_when_inactive(&mut self, pause: bool) {
        self.set_global(PauseWhenInactive(pause));
    }
}
