//! What makes it alive — a wobble in the outline, a breath in the body, a
//! drift in the gaze, a blink. Every one is a function of `t` and nothing is
//! stored between frames, so a paused page is an honest still.

use crate::avatar::shape::Shape;
use std::f32::consts::PI;

/// Seconds between blinks, and how long one takes.
const BLINK_EVERY: f32 = 4.4;
const BLINK_FOR: f32 = 0.16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// Radians per second the lobe phases travel.
    pub wobble: f32,
    /// How much the whole body breathes, as a fraction of its radius.
    pub breathe: f32,
    /// How far the gaze wanders, in units of the body radius.
    pub drift: f32,
    pub blink: bool,
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
    };
    pub const ALIVE: Self = Self {
        wobble: 0.34,
        breathe: 0.014,
        drift: 0.03,
        blink: true,
    };

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
        let lid = if self.blink {
            let p = (t / BLINK_EVERY).fract() * BLINK_EVERY;
            if p < BLINK_FOR {
                (PI * p / BLINK_FOR).sin()
            } else {
                0.0
            }
        } else {
            0.0
        };
        Beat {
            scale: 1.0 + self.breathe * (0.9 * t).sin(),
            gaze: (
                self.drift * (0.6 * (0.31 * t).sin() + 0.4 * (0.17 * t + 1.3).sin()),
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
