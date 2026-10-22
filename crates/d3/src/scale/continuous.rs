//! d3-scale's continuous scales: `linear`, `log`, `pow`, `sqrt`, `symlog`,
//! `time` and `utc`.

use crate::{
    array,
    format::number::{self, Number},
    interpolate,
    time::{self, Interval, date::LocalZone},
};
use std::f64::consts::E;

/// How a continuous scale bends its domain before mapping it straight onto
/// its range.
#[derive(Clone, Debug, PartialEq)]
pub enum Transform {
    Linear,
    Log {
        base: f64,
    },
    /// `sqrt` is an exponent of 0.5.
    Pow {
        exponent: f64,
    },
    Symlog {
        constant: f64,
    },
    /// Linear in milliseconds since the epoch, with ticks and a nice domain
    /// on the zone's calendar: d3's `scaleTime`, or `scaleUtc` in
    /// [`LocalZone::utc`].
    Time(LocalZone),
}

impl Transform {
    /// The transform and its inverse, a log scale's reflected for a domain
    /// below zero.
    pub(crate) fn forward(&self, x: f64, negative: bool) -> f64 {
        match *self {
            Transform::Linear | Transform::Time(_) => x,
            Transform::Log { .. } if negative => -(-x).ln(),
            Transform::Log { .. } => x.ln(),
            Transform::Pow { exponent: 1.0 } => x,
            Transform::Pow { exponent: 0.5 } => match x < 0.0 {
                true => -(-x).sqrt(),
                false => x.sqrt(),
            },
            Transform::Pow { exponent } => match x < 0.0 {
                true => -number::pow(-x, exponent),
                false => number::pow(x, exponent),
            },
            Transform::Symlog { constant } => number::sign(x) * (x / constant).abs().ln_1p(),
        }
    }

    fn backward(&self, y: f64, negative: bool) -> f64 {
        match *self {
            Transform::Linear | Transform::Time(_) => y,
            Transform::Log { .. } if negative => -(-y).exp(),
            Transform::Log { .. } => y.exp(),
            Transform::Pow { exponent: 1.0 } => y,
            Transform::Pow { exponent: 0.5 } => match y < 0.0 {
                true => -y * y,
                false => y * y,
            },
            Transform::Pow { exponent } => match y < 0.0 {
                true => -number::pow(-y, 1.0 / exponent),
                false => number::pow(y, 1.0 / exponent),
            },
            Transform::Symlog { constant } => number::sign(y) * y.abs().exp_m1() * constant,
        }
    }
}

/// A continuous scale. Where the domain and range differ in length, the
/// shorter sets how many pieces there are.
#[derive(Clone, Debug, PartialEq)]
pub struct Continuous {
    pub transform: Transform,
    pub domain: Vec<f64>,
    pub range: Vec<f64>,
    /// Whether values outside the domain map to the range's ends.
    pub clamp: bool,
    /// Whether outputs round to whole numbers, as `interpolateRound` does.
    pub round: bool,
}

/// `normalize(a, b)(x)`.
fn normalize(a: f64, b: f64, x: f64) -> f64 {
    let span = b - a;
    match span != 0.0 && !span.is_nan() {
        true => (x - a) / span,
        false if span.is_nan() => f64::NAN,
        false => 0.5,
    }
}

/// d3's `bimap` or `polymap` from `domain` onto `range` at `x`.
fn piecewise(domain: &[f64], range: &[f64], x: f64, round: bool) -> f64 {
    let n = domain.len().min(range.len());
    if n < 2 {
        return range.first().copied().unwrap_or(f64::NAN);
    }
    let lerp = |a: f64, b: f64, t: f64| match round {
        true => interpolate::round(a, b, t),
        false => interpolate::number(a, b, t),
    };
    if n == 2 {
        let (d0, d1, r0, r1) = (domain[0], domain[1], range[0], range[1]);
        return match d1 < d0 {
            true => lerp(r1, r0, normalize(d1, d0, x)),
            false => lerp(r0, r1, normalize(d0, d1, x)),
        };
    }
    let j = n - 1;
    let (domain, range): (Vec<f64>, Vec<f64>) = match domain[j] < domain[0] {
        true => (
            domain[..n].iter().rev().copied().collect(),
            range[..n].iter().rev().copied().collect(),
        ),
        false => (domain[..n].to_vec(), range[..n].to_vec()),
    };
    let i = array::bisect_right(&domain, x, 1, j) - 1;
    lerp(
        range[i],
        range[i + 1],
        normalize(domain[i], domain[i + 1], x),
    )
}

/// d3's `logp(base)`, reflected for a domain below zero.
fn logs(base: f64, negative: bool, x: f64) -> f64 {
    let log = |x: f64| {
        if base == E {
            x.ln()
        } else if base == 10.0 {
            x.log10()
        } else if base == 2.0 {
            x.log2()
        } else {
            x.ln() / base.ln()
        }
    };
    match negative {
        true => -log(-x),
        false => log(x),
    }
}

/// d3's `powp(base)`, reflected for a domain below zero.
fn pows(base: f64, negative: bool, x: f64) -> f64 {
    let pow = |x: f64| {
        if base == 10.0 {
            pow10(x)
        } else if base == E {
            x.exp()
        } else {
            number::pow(base, x)
        }
    };
    match negative {
        true => -pow(-x),
        false => pow(x),
    }
}

/// `10^x` as d3's log scale reads `1e` and `x` back as a number.
fn pow10(x: f64) -> f64 {
    match x.is_finite() {
        true => Number::from(format!("1e{}", Number(x)).as_str()).0,
        false if x < 0.0 => 0.0,
        false => x,
    }
}

impl Continuous {
    /// `transform` from `domain` onto `range`, unclamped and unrounded. A
    /// time scale's domain is held as dates are: whole milliseconds.
    pub fn new(transform: Transform, domain: Vec<f64>, range: Vec<f64>) -> Self {
        let domain = match &transform {
            Transform::Time(zone) => domain.iter().map(|&d| zone.date(d).time()).collect(),
            _ => domain,
        };
        Self {
            transform,
            domain,
            range,
            clamp: false,
            round: false,
        }
    }

    /// Whether a log scale's domain is below zero, which reflects it.
    fn negative(&self) -> bool {
        matches!(self.transform, Transform::Log { .. })
            && self.domain.first().is_some_and(|d| *d < 0.0)
    }

    /// `x` held inside the domain's ends, when the scale clamps.
    fn clamped(&self, x: f64) -> f64 {
        let n = self.domain.len().min(self.range.len());
        if !self.clamp || n == 0 {
            return x;
        }
        let (a, b) = (self.domain[0], self.domain[n - 1]);
        let (lo, hi) = if a > b { (b, a) } else { (a, b) };
        number::max(lo, number::min(hi, x))
    }

    fn transformed(&self) -> Vec<f64> {
        let negative = self.negative();
        self.domain
            .iter()
            .map(|&d| self.transform.forward(d, negative))
            .collect()
    }

    /// Where `x` lands in the range. `NaN` for `NaN`.
    pub fn at(&self, x: f64) -> f64 {
        if x.is_nan() {
            return f64::NAN;
        }
        let x = self.transform.forward(self.clamped(x), self.negative());
        piecewise(&self.transformed(), &self.range, x, self.round)
    }

    /// The domain value that lands at `y`.
    pub fn invert(&self, y: f64) -> f64 {
        let x = piecewise(&self.range, &self.transformed(), y, false);
        self.clamped(self.transform.backward(x, self.negative()))
    }

    fn ends(&self) -> (f64, f64) {
        match self.domain.as_slice() {
            [] => (f64::NAN, f64::NAN),
            [first, .., last] => (*first, *last),
            [only] => (*only, *only),
        }
    }

    /// d3's `scale.nice(interval)`: the domain's ends floored and ceiled to
    /// `interval`, in a time scale's zone.
    pub fn nice_to(&mut self, interval: &Interval) {
        let Transform::Time(zone) = &self.transform else {
            return;
        };
        let Some(last) = self.domain.len().checked_sub(1) else {
            return;
        };
        let (i0, i1) = match self.domain[last] < self.domain[0] {
            true => (last, 0),
            false => (0, last),
        };
        let edge = |at: f64, ceil: bool| {
            let mut d = zone.date(at);
            match ceil {
                true => interval.ceil(&mut d),
                false => interval.floor(&mut d),
            }
            d.time()
        };
        let (start, stop) = (self.domain[i0], self.domain[i1]);
        self.domain[i0] = edge(start, false);
        self.domain[i1] = edge(stop, true);
    }

    /// Vega's `padDomain`: the domain's ends moved apart, through the
    /// transform about their middle, so `pixels` of the range lie past each.
    pub fn pad(&mut self, pixels: f64) {
        let (Some(&r0), Some(&r1)) = (self.range.first(), self.range.last()) else {
            return;
        };
        let Some(last) = self.domain.len().checked_sub(1) else {
            return;
        };
        let span = (r1 - r0).abs();
        let scale = span / (span - 2.0 * pixels);
        let negative = self.negative();
        let (d0, d1) = (
            self.transform.forward(self.domain[0], negative),
            self.transform.forward(self.domain[last], negative),
        );
        let middle = (d0 + d1) / 2.0;
        self.domain[0] = self
            .transform
            .backward(middle + (d0 - middle) * scale, negative);
        self.domain[last] = self
            .transform
            .backward(middle + (d1 - middle) * scale, negative);
    }

    /// `scale.ticks(count)`: about `count` round values across the domain.
    pub fn ticks(&self, count: f64) -> Vec<f64> {
        let (d0, d1) = self.ends();
        match &self.transform {
            Transform::Log { base } => self.log_ticks(*base, count),
            Transform::Time(zone) => time::ticks(d0, d1, count, zone),
            _ => array::ticks(d0, d1, count),
        }
    }

    fn log_ticks(&self, base: f64, count: f64) -> Vec<f64> {
        let (mut u, mut v) = self.ends();
        let reverse = v < u;
        if reverse {
            (u, v) = (v, u);
        }
        let negative = self.negative();
        let logs = |x: f64| logs(base, negative, x);
        let pows = |x: f64| pows(base, negative, x);
        let (mut i, mut j) = (logs(u), logs(v));
        let n = count;
        let mut z = Vec::new();
        if base % 1.0 == 0.0 && j - i < n {
            (i, j) = (i.floor(), j.ceil());
            let mut i = i;
            if u > 0.0 {
                while i <= j {
                    let mut k = 1.0;
                    while k < base {
                        let t = match i < 0.0 {
                            true => k / pows(-i),
                            false => k * pows(i),
                        };
                        k += 1.0;
                        if t < u {
                            continue;
                        }
                        if t > v {
                            break;
                        }
                        z.push(t);
                    }
                    i += 1.0;
                }
            } else {
                while i <= j {
                    let mut k = base - 1.0;
                    while k >= 1.0 {
                        let t = match i > 0.0 {
                            true => k / pows(-i),
                            false => k * pows(i),
                        };
                        k -= 1.0;
                        if t < u {
                            continue;
                        }
                        if t > v {
                            break;
                        }
                        z.push(t);
                    }
                    i += 1.0;
                }
            }
            if ((z.len() * 2) as f64) < n {
                z = array::ticks(u, v, n);
            }
        } else {
            z = array::ticks(i, j, (j - i).min(n))
                .into_iter()
                .map(pows)
                .collect();
        }
        if reverse {
            z.reverse();
        }
        z
    }

    /// `scale.nice(count)`: the domain's ends widened to round values, on
    /// the calendar for time, to powers of the base for log.
    pub fn nice(&mut self, count: f64) {
        let last = self.domain.len().saturating_sub(1);
        if self.domain.is_empty() {
            return;
        }
        let (mut i0, mut i1) = (0, last);
        if self.domain[i1] < self.domain[i0] {
            (i0, i1) = (i1, i0);
        }
        let (start, stop) = (self.domain[i0], self.domain[i1]);
        match &self.transform {
            Transform::Log { base } => {
                let negative = self.negative();
                self.domain[i0] = pows(*base, negative, logs(*base, negative, start).floor());
                self.domain[i1] = pows(*base, negative, logs(*base, negative, stop).ceil());
            }
            Transform::Time(zone) => {
                if let Some(interval) =
                    time::tick_interval(self.domain[0], self.domain[last], count, zone)
                {
                    self.nice_to(&interval);
                }
            }
            _ => {
                let (mut start, mut stop) = (start, stop);
                let mut previous = None;
                for _ in 0..10 {
                    let step = array::tick_increment(start, stop, count);
                    if previous == Some(step) {
                        self.domain[i0] = start;
                        self.domain[i1] = stop;
                        return;
                    } else if step > 0.0 {
                        start = (start / step).floor() * step;
                        stop = (stop / step).ceil() * step;
                    } else if step < 0.0 {
                        start = (start * step).ceil() / step;
                        stop = (stop * step).floor() / step;
                    } else {
                        return;
                    }
                    previous = Some(step);
                }
            }
        }
    }
}
