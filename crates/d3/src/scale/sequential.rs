//! d3-scale's sequential and diverging scales: a domain onto `0..=1`, which
//! an interpolator takes on to a colour.

use crate::scale::Transform;

/// d3's `scaleSequential` and its log, pow, sqrt and symlog kinds.
#[derive(Clone, Debug, PartialEq)]
pub struct Sequential {
    pub transform: Transform,
    pub domain: [f64; 2],
    pub clamp: bool,
}

/// Whether a log scale's domain starts below zero, which reflects it.
fn negative(transform: &Transform, first: f64) -> bool {
    matches!(transform, Transform::Log { .. }) && first < 0.0
}

impl Sequential {
    /// Where `x` falls, 0 at the domain's start and 1 at its end. `NaN` for
    /// `NaN`.
    pub fn at(&self, x: f64) -> f64 {
        if x.is_nan() {
            return f64::NAN;
        }
        let negative = negative(&self.transform, self.domain[0]);
        let forward = |x: f64| self.transform.forward(x, negative);
        let (t0, t1) = (forward(self.domain[0]), forward(self.domain[1]));
        if t0 == t1 {
            return 0.5;
        }
        let k10 = 1.0 / (t1 - t0);
        let t = (forward(x) - t0) * k10;
        match self.clamp {
            true => t.clamp(0.0, 1.0),
            false => t,
        }
    }
}

/// d3's `scaleDiverging` and its log, pow, sqrt and symlog kinds: the
/// domain's middle at 0.5.
#[derive(Clone, Debug, PartialEq)]
pub struct Diverging {
    pub transform: Transform,
    pub domain: [f64; 3],
    pub clamp: bool,
}

impl Diverging {
    pub fn at(&self, x: f64) -> f64 {
        if x.is_nan() {
            return f64::NAN;
        }
        let negative = negative(&self.transform, self.domain[0]);
        let forward = |x: f64| self.transform.forward(x, negative);
        let [t0, t1, t2] = self.domain.map(forward);
        let k10 = if t0 == t1 { 0.0 } else { 0.5 / (t1 - t0) };
        let k21 = if t1 == t2 { 0.0 } else { 0.5 / (t2 - t1) };
        let s = if t1 < t0 { -1.0 } else { 1.0 };
        let x = forward(x);
        let t = 0.5 + (x - t1) * if s * x < s * t1 { k10 } else { k21 };
        match self.clamp {
            true => t.clamp(0.0, 1.0),
            false => t,
        }
    }
}
