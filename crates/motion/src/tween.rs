//! [`Tween`] — a box moving between two places along a timeline.

use crate::MotionSpec;
use gpui::{App, Bounds, Pixels, size};
use web_time::Instant;

/// A box moving from `from` to `to` along `spec`, set out at `since` on the
/// background executor's clock.
#[derive(Clone, Copy, Debug)]
pub struct Tween {
    pub spec: MotionSpec,
    pub from: Bounds<Pixels>,
    pub to: Bounds<Pixels>,
    pub since: Instant,
}

impl Tween {
    /// Eased progress (0..1) now; 1 under reduced motion.
    pub fn progress(&self, cx: &App) -> f32 {
        if cx.reduce_motion() {
            return 1.0;
        }
        let now = cx.background_executor().now();
        self.spec
            .progress_after(now.saturating_duration_since(self.since))
    }

    /// The box at progress `t`.
    pub fn at(&self, t: f32) -> Bounds<Pixels> {
        let (from, to) = (self.from, self.to);
        Bounds::new(
            from.origin + (to.origin - from.origin) * t,
            size(
                from.size.width + (to.size.width - from.size.width) * t,
                from.size.height + (to.size.height - from.size.height) * t,
            ),
        )
    }

    /// The box now.
    pub fn bounds(&self, cx: &App) -> Bounds<Pixels> {
        self.at(self.progress(cx))
    }
}
