//! d3-array's `ticks`, `tickIncrement`, `tickStep`, `nice`, `bisectRight`
//! and `quantileSorted`.

use crate::math;

const E10: f64 = 7.0710678118654755;
const E5: f64 = 3.1622776601683795;
const E2: f64 = std::f64::consts::SQRT_2;

/// The first and last tick's index and the increment: a step of
/// `10^power * factor` as is, a step below one as the negative of its
/// inverse.
fn tick_spec(start: f64, stop: f64, count: f64) -> (f64, f64, f64) {
    let step = (stop - start) / count.max(0.0);
    let power = step.log10().floor();
    let error = step / math::pow(10.0, power);
    let factor = if error >= E10 {
        10.0
    } else if error >= E5 {
        5.0
    } else if error >= E2 {
        2.0
    } else {
        1.0
    };
    let (mut i1, mut i2, inc);
    if power < 0.0 {
        let mut inverse = math::pow(10.0, -power) / factor;
        i1 = math::round(start * inverse);
        i2 = math::round(stop * inverse);
        if i1 / inverse < start {
            i1 += 1.0;
        }
        if i2 / inverse > stop {
            i2 -= 1.0;
        }
        inverse = -inverse;
        inc = inverse;
    } else {
        inc = math::pow(10.0, power) * factor;
        i1 = math::round(start / inc);
        i2 = math::round(stop / inc);
        if i1 * inc < start {
            i1 += 1.0;
        }
        if i2 * inc > stop {
            i2 -= 1.0;
        }
    }
    if i2 < i1 && (0.5..2.0).contains(&count) {
        return tick_spec(start, stop, count * 2.0);
    }
    (i1, i2, inc)
}

/// `d3.ticks(start, stop, count)`: about `count` round values from `start`
/// to `stop`, in their order.
pub fn ticks(start: f64, stop: f64, count: f64) -> Vec<f64> {
    if count.is_nan() || count <= 0.0 {
        return Vec::new();
    }
    if start == stop {
        return vec![start];
    }
    let reverse = stop < start;
    let (i1, i2, inc) = match reverse {
        true => tick_spec(stop, start, count),
        false => tick_spec(start, stop, count),
    };
    if i1.is_nan() || i2.is_nan() || i2 < i1 {
        return Vec::new();
    }
    let n = (i2 - i1 + 1.0) as usize;
    (0..n)
        .map(|i| {
            let index = match reverse {
                true => i2 - i as f64,
                false => i1 + i as f64,
            };
            match inc < 0.0 {
                true => index / -inc,
                false => index * inc,
            }
        })
        .collect()
}

/// `d3.tickIncrement(start, stop, count)`: the tick step, a step below one
/// as the negative of its inverse. `start` is at most `stop`.
pub fn tick_increment(start: f64, stop: f64, count: f64) -> f64 {
    tick_spec(start, stop, count).2
}

/// `d3.tickStep(start, stop, count)`: the tick step, negative for a
/// `stop` before `start`.
pub fn tick_step(start: f64, stop: f64, count: f64) -> f64 {
    let reverse = stop < start;
    let inc = match reverse {
        true => tick_increment(stop, start, count),
        false => tick_increment(start, stop, count),
    };
    let step = match inc < 0.0 {
        true => 1.0 / -inc,
        false => inc,
    };
    match reverse {
        true => -step,
        false => step,
    }
}

/// `d3.nice(start, stop, count)`: `start..stop` widened to the ticks either
/// side.
pub fn nice(mut start: f64, mut stop: f64, count: f64) -> (f64, f64) {
    let mut previous = None;
    loop {
        let step = tick_increment(start, stop, count);
        if previous == Some(step) || step == 0.0 || !step.is_finite() {
            return (start, stop);
        }
        if step > 0.0 {
            start = (start / step).floor() * step;
            stop = (stop / step).ceil() * step;
        } else {
            start = (start * step).ceil() / step;
            stop = (stop * step).floor() / step;
        }
        previous = Some(step);
    }
}

/// `d3.bisectRight(values, x, lo, hi)`: where `x` goes in ascending
/// `values[lo..hi]`, after any equal to it.
pub fn bisect_right(values: &[f64], x: f64, lo: usize, hi: usize) -> usize {
    if x.is_nan() {
        return hi;
    }
    let (mut lo, mut hi) = (lo, hi);
    while lo < hi {
        let mid = (lo + hi) / 2;
        // d3 compares with `ascending`, which puts NaN nowhere.
        if values[mid] <= x {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// `d3.quantileSorted(values, p)`: the `p`-quantile of ascending `values`
/// by R-7. `NaN` for none.
pub fn quantile_sorted(values: &[f64], p: f64) -> f64 {
    let n = values.len();
    if n == 0 || p.is_nan() {
        return f64::NAN;
    }
    if p <= 0.0 || n < 2 {
        return values[0];
    }
    if p >= 1.0 {
        return values[n - 1];
    }
    let i = (n - 1) as f64 * p;
    let i0 = i.floor() as usize;
    let (value0, value1) = (values[i0], values[i0 + 1]);
    value0 + (value1 - value0) * (i - i0 as f64)
}
