//! Pele's `diagrams/xychart/ticks.ts`.

use crate::js;

/// A step of 1, 2 or 5 times a power of ten that divides `span` into about
/// `count` parts. A step below one comes back as the negative of its inverse,
/// which is a whole number, so that tick values are found by division and
/// print without rounding noise (0.3, not 0.30000000000000004).
pub fn tick_step(span: f64, count: f64) -> f64 {
    let raw = span / count.max(1.0);
    let power = raw.log10().floor();
    let unit = raw / 10f64.powf(power);
    let factor = if unit >= 7.07 {
        10.0
    } else if unit >= 3.16 {
        5.0
    } else if unit >= 1.41 {
        2.0
    } else {
        1.0
    };
    if power < 0.0 {
        -10f64.powf(-power) / factor
    } else {
        factor * 10f64.powf(power)
    }
}

pub fn tick_index(step: f64, value: f64) -> f64 {
    if step > 0.0 {
        value / step
    } else {
        value * -step
    }
}

pub fn tick_value(step: f64, index: f64) -> f64 {
    if step > 0.0 {
        index * step
    } else {
        index / -step
    }
}

/// The multiples of a step from `lo` to `hi`, both included.
pub fn ticks_between(lo: f64, hi: f64, step: f64) -> Vec<f64> {
    let mut first = js::round(tick_index(step, lo));
    if tick_value(step, first) < lo {
        first += 1.0;
    }
    let mut last = js::round(tick_index(step, hi));
    if tick_value(step, last) > hi {
        last -= 1.0;
    }
    let mut out = Vec::new();
    let mut i = first;
    while i <= last && out.len() < 100 {
        out.push(tick_value(step, i));
        i += 1.0;
    }
    out
}
