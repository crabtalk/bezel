//! What makes it alive — a wobble in the outline, a breath in the body, a
//! drift in the gaze, a blink. Every one is a function of `t` and nothing is
//! stored between frames, so a paused page is an honest still.

use crate::avatar::shape::Shape;
use std::f32::consts::PI;

/// Seconds between blinks, and how long one takes.
const BLINK_EVERY: f32 = 4.4;
const BLINK_FOR: f32 = 0.16;
/// Seconds the gaze takes to sweep side to side and back.
const SCAN_EVERY: f32 = 1.2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// Radians per second the lobe phases travel.
    pub wobble: f32,
    /// How much the whole body breathes, as a fraction of its radius.
    pub breathe: f32,
    /// How far the gaze wanders, in units of the body radius.
    pub drift: f32,
    pub blink: bool,
    /// How far the gaze sweeps side to side, in units of the body radius. A
    /// sweep of a quarter radius moves the eyes a cell in a [`mascot`].
    ///
    /// [`mascot`]: crate::avatar::mascot
    pub scan: f32,
    /// How shut the eyes rest, 0 open to 1 closed. A blink shuts them further
    /// and never opens them past this.
    pub lid: f32,
}

impl Default for Motion {
    fn default() -> Self {
        Self::ALIVE
    }
}

impl Motion {
    pub const STILL: Self = Self {
        wobble: 0.0,
        breathe: 0.0,
        drift: 0.0,
        blink: false,
        scan: 0.0,
        lid: 0.0,
    };
    pub const ALIVE: Self = Self {
        wobble: 0.34,
        breathe: 0.014,
        drift: 0.03,
        blink: true,
        scan: 0.0,
        lid: 0.0,
    };
    /// Alive, its gaze sweeping side to side.
    pub const BUSY: Self = Self {
        scan: 0.3,
        ..Self::ALIVE
    };
    /// Eyes shut, the body still but for its breath.
    pub const ASLEEP: Self = Self {
        wobble: 0.0,
        breathe: 0.014,
        drift: 0.0,
        blink: false,
        scan: 0.0,
        lid: 1.0,
    };

    /// How far the gaze can wander from rest, drift and sweep together.
    pub fn reach(&self) -> f32 {
        self.drift + self.scan
    }

    /// The gaze's side-to-side offset at `t`.
    pub fn sweep(&self, t: f32) -> f32 {
        self.scan * (2.0 * PI * t / SCAN_EVERY).sin()
    }

    /// The outline at `t`. Each harmonic travels at its own rate, which is what
    /// keeps the wobble from reading as the whole body rotating.
    pub fn shape(&self, mut shape: Shape, t: f32) -> Shape {
        if self.wobble == 0.0 {
            return shape;
        }
        for l in &mut shape.lobes {
            l.phase += t * self.wobble * (0.7 + 0.13 * l.k as f32);
        }
        shape
    }

    /// Everything that moves without changing the outline.
    pub fn beat(&self, t: f32) -> Beat {
        let blink = if self.blink {
            let p = (t / BLINK_EVERY).fract() * BLINK_EVERY;
            if p < BLINK_FOR {
                (PI * p / BLINK_FOR).sin()
            } else {
                0.0
            }
        } else {
            0.0
        };
        let lid = blink.max(self.lid.clamp(0.0, 1.0));
        Beat {
            scale: 1.0 + self.breathe * (0.9 * t).sin(),
            gaze: (
                self.drift * (0.6 * (0.31 * t).sin() + 0.4 * (0.17 * t + 1.3).sin())
                    + self.sweep(t),
                self.drift * (0.5 * (0.23 * t + 0.7).sin() + 0.5 * (0.13 * t).sin()),
            ),
            lid,
        }
    }
}

/// The moving parts at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beat {
    pub scale: f32,
    pub gaze: (f32, f32),
    /// How shut the eyes are: 0 open, 1 closed.
    pub lid: f32,
}
