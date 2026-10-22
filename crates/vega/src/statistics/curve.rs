//! vega-statistics' `sampleCurve`: a function sampled over an interval.

use d3::math;
use std::f64::consts::PI;

/// Half a degree: a midpoint turning the curve by less is not kept.
const MIN_RADIANS: f64 = 0.5 * PI / 180.0;

/// `[x, f(x)]` from `low` to `high`, both ends included. `min_steps` even
/// steps, then each step halved while its midpoint bends the curve and stays
/// at least `1 / max_steps` of the interval from its start. Equal step counts
/// sample exactly `steps + 1` even points. 0 reads as 25 and 200.
pub fn sample(
    f: impl Fn(f64) -> f64,
    [low, high]: [f64; 2],
    min_steps: usize,
    max_steps: usize,
) -> Vec<[f64; 2]> {
    let min_steps = if min_steps == 0 { 25 } else { min_steps };
    let max_steps = min_steps.max(if max_steps == 0 { 200 } else { max_steps });
    let point = |x: f64| [x, f(x)];
    let span = high - low;
    let stop = span / max_steps as f64;
    let mut prev = vec![point(low)];

    if min_steps == max_steps {
        for i in 1..max_steps {
            prev.push(point(low + (i as f64 / min_steps as f64) * span));
        }
        prev.push(point(high));
        return prev;
    }

    let mut next = vec![point(high)];
    for i in (1..min_steps).rev() {
        next.push(point(low + (i as f64 / min_steps as f64) * span));
    }

    let mut p0 = prev[0];
    let sx = 1.0 / span;
    let (mut y_min, mut y_max) = (p0[1], p0[1]);
    for [_, y] in &next {
        if *y < y_min {
            y_min = *y;
        }
        if *y > y_max {
            y_max = *y;
        }
    }
    let sy = 1.0 / (y_max - y_min);
    let angle = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        let a0 = math::atan2(sy * (r[1] - p[1]), sx * (r[0] - p[0]));
        let a1 = math::atan2(sy * (q[1] - p[1]), sx * (q[0] - p[0]));
        (a0 - a1).abs()
    };

    while let Some(&p1) = next.last() {
        let pm = point((p0[0] + p1[0]) / 2.0);
        if pm[0] - p0[0] >= stop && angle(p0, pm, p1) > MIN_RADIANS {
            next.push(pm);
        } else {
            p0 = p1;
            prev.push(p1);
            next.pop();
        }
    }
    prev
}

/// `[min, max]` of the values that are not `NaN`. `None` if none are.
pub fn extent(values: impl IntoIterator<Item = f64>) -> Option<[f64; 2]> {
    values
        .into_iter()
        .filter(|v| !v.is_nan())
        .fold(None, |extent, v| match extent {
            None => Some([v, v]),
            Some([low, high]) => Some([low.min(v), high.max(v)]),
        })
}
