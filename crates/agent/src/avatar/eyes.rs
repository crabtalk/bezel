//! The eye rig, in units of the body radius, and the one solve that keeps it
//! inside whatever silhouette it was handed.

use crate::avatar::shape::Shape;

/// Clearance the fit aims for: the eye cluster's furthest corner sits this far
/// along the outline's reach, never at it.
const MARGIN: f32 = 0.86;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eyes {
    /// Half-width of one eye.
    pub rx: f32,
    /// Height against width. Past ~1.5 the eye reads as a capsule.
    pub ratio: f32,
    /// Half the centre-to-centre distance.
    pub gap: f32,
    /// How high on the body the pair sits.
    pub rise: f32,
    /// Degrees, mirrored between the two.
    pub lean: f32,
    /// Superellipse exponent — 2 is an ellipse, 5 a rounded slab.
    pub n: f32,
}

impl Default for Eyes {
    fn default() -> Self {
        Self::CALM
    }
}

impl Eyes {
    pub const CALM: Self = Self {
        rx: 0.15,
        ratio: 1.9,
        gap: 0.34,
        rise: 0.04,
        lean: 0.0,
        n: 4.0,
    };
    pub const WIDE: Self = Self {
        rx: 0.2,
        ratio: 1.15,
        gap: 0.4,
        ..Self::CALM
    };
    pub const KEEN: Self = Self {
        rx: 0.12,
        ratio: 2.8,
        lean: 12.0,
        ..Self::CALM
    };
    pub const SLEEPY: Self = Self {
        rx: 0.19,
        ratio: 0.42,
        rise: 0.0,
        n: 5.0,
        ..Self::CALM
    };
    pub const TALL: Self = Self {
        rx: 0.11,
        ratio: 3.2,
        gap: 0.32,
        ..Self::CALM
    };
    /// `n` near 2 is a true ellipse, which at this ratio is a circle.
    pub const DOTS: Self = Self {
        rx: 0.11,
        ratio: 1.0,
        gap: 0.3,
        n: 2.2,
        ..Self::CALM
    };
    /// A lean only reads on an elongated eye — a round one looks the same at
    /// every angle.
    pub const SLY: Self = Self {
        rx: 0.12,
        ratio: 2.5,
        lean: 16.0,
        ..Self::CALM
    };
    pub const CLOSE: Self = Self {
        gap: 0.22,
        ..Self::CALM
    };
    pub const FAR: Self = Self {
        gap: 0.5,
        ..Self::CALM
    };
    pub const LOW: Self = Self {
        rise: -0.16,
        ..Self::CALM
    };

    pub const PRESETS: [(&'static str, Self); 10] = [
        ("calm", Self::CALM),
        ("wide", Self::WIDE),
        ("keen", Self::KEEN),
        ("tall", Self::TALL),
        ("dots", Self::DOTS),
        ("sly", Self::SLY),
        ("sleepy", Self::SLEEPY),
        ("close", Self::CLOSE),
        ("far", Self::FAR),
        ("low", Self::LOW),
    ];
}

impl From<u64> for Eyes {
    fn from(seed: u64) -> Self {
        // Offset so a name's eyes do not follow its silhouette in lockstep.
        let mut h = seed ^ 0x5bf0_3635_ca62_9d7f;
        let mut unit = || {
            h = (h ^ (h >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);
            (h >> 40) as f32 / 16_777_216.0
        };
        let range = |u: f32, lo: f32, hi: f32| lo + u * (hi - lo);
        Self {
            rx: range(unit(), 0.12, 0.19),
            ratio: range(unit(), 1.1, 2.6),
            gap: range(unit(), 0.28, 0.42),
            rise: range(unit(), -0.02, 0.12),
            lean: range(unit(), -12.0, 12.0),
            n: range(unit(), 2.6, 5.5),
        }
    }
}

/// One eye where it lands, in the same unit space as [`Shape::outline`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eye {
    pub cx: f32,
    pub cy: f32,
    pub rx: f32,
    pub ry: f32,
    pub rot: f32,
    pub n: f32,
}

impl Eyes {
    /// Fits the pair against the body's real radius in each eye's own
    /// direction.
    ///
    /// Solved once and never again: a fit that re-ran per frame would resize
    /// the eyes as the gaze drifted, which is the tremble bloub documents. So
    /// `drift` — how far motion may later wander — is spent here, at the worst
    /// case over everywhere it can reach.
    pub fn place(&self, shape: &Shape, drift: f32) -> [Eye; 2] {
        let cy = -self.rise;
        let reach = self.rx.hypot(self.rx * self.ratio);
        let wander = drift * std::f32::consts::SQRT_2;
        let fit = [-self.gap, self.gap]
            .iter()
            .map(|&cx| {
                let room = [
                    (-drift, -drift),
                    (drift, -drift),
                    (-drift, drift),
                    (drift, drift),
                ]
                .iter()
                .map(|(dx, dy)| shape.reach(cx + dx, cy + dy))
                .fold(f32::INFINITY, f32::min)
                    * MARGIN;
                let want = cx.hypot(cy) + reach;
                if want + wander > room {
                    (room - wander) / want
                } else {
                    1.0
                }
            })
            .fold(1.0f32, f32::min);

        [-1.0f32, 1.0].map(|side| Eye {
            cx: side * self.gap * fit,
            cy: cy * fit,
            rx: self.rx * fit,
            ry: self.rx * self.ratio * fit,
            rot: side * self.lean,
            n: self.n,
        })
    }
}
