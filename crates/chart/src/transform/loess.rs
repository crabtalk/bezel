//! Vega-Lite's `loess` transform: locally-weighted regression.

use crate::{data::Data, transform::Groups};
use gpui::SharedString;
use js::array;

/// Robustness iterations after the first fit.
const ITERATIONS: usize = 2;
const EPSILON: f64 = 1e-12;

/// A smoothed `field` over `on` per group: one row per distinct value of `on`,
/// ascending, its fits averaged.
#[derive(Clone, Debug, PartialEq)]
pub struct Loess {
    /// The smoothed field.
    pub field: SharedString,
    /// The predictor field.
    pub on: SharedString,
    pub groupby: Vec<SharedString>,
    /// The share of the group's points each local fit takes in, at least two.
    /// 0 reads as 0.3.
    pub bandwidth: f64,
    /// The predictor's and the fit's fields, Vega-Lite's `as`.
    pub output: [SharedString; 2],
}

impl Loess {
    /// Vega-Lite's defaults: a bandwidth of 0.3, output as `on` and `field`.
    pub fn new(field: impl Into<SharedString>, on: impl Into<SharedString>) -> Self {
        let (field, on) = (field.into(), on.into());
        Self {
            output: [on.clone(), field.clone()],
            field,
            on,
            groupby: Vec::new(),
            bandwidth: 0.3,
        }
    }

    pub fn apply(&self, data: &Data) -> Data {
        let xs = data.numbers(&self.on);
        let ys = data.numbers(&self.field);
        let bandwidth = if self.bandwidth == 0.0 || self.bandwidth.is_nan() {
            0.3
        } else {
            self.bandwidth
        };
        let groups = Groups::of(data, &self.groupby);
        let points: Vec<Vec<[f64; 2]>> = groups
            .rows
            .iter()
            .map(|rows| {
                let mut pairs: Vec<[f64; 2]> = rows
                    .iter()
                    .map(|row| [xs[*row], ys[*row]])
                    .filter(|[x, y]| !x.is_nan() && !y.is_nan())
                    .collect();
                pairs.sort_by(|a, b| a[0].total_cmp(&b[0]));
                smooth(&pairs, bandwidth)
            })
            .collect();
        groups.table(data, &points, &self.output)
    }
}

/// Vega's `regressionLoess` over `pairs` ascending by x.
fn smooth(pairs: &[[f64; 2]], bandwidth: f64) -> Vec<[f64; 2]> {
    let n = pairs.len();
    let (mut ux, mut uy) = (0.0, 0.0);
    for (i, [x, y]) in pairs.iter().enumerate() {
        let i = (i + 1) as f64;
        ux += (x - ux) / i;
        uy += (y - uy) / i;
    }
    let xv: Vec<f64> = pairs.iter().map(|[x, _]| x - ux).collect();
    let yv: Vec<f64> = pairs.iter().map(|[_, y]| y - uy).collect();
    // Out of range reads as `NaN`, as an array past its end does in Vega.
    let at = |values: &[f64], i: usize| values.get(i).copied().unwrap_or(f64::NAN);
    let neighbours = 2.max((bandwidth * n as f64) as usize);
    let mut fit = vec![0.0; n];
    let mut residuals = vec![0.0; n];
    let mut robust = vec![1.0; n];

    for iteration in 0..=ITERATIONS {
        let [mut left, mut right] = [0, neighbours - 1];
        for i in 0..n {
            let dx = xv[i];
            let edge = if dx - at(&xv, left) > at(&xv, right) - dx {
                left
            } else {
                right
            };
            let span = at(&xv, edge) - dx;
            let span = if span == 0.0 || span.is_nan() {
                1.0
            } else {
                span
            };
            let denom = 1.0 / span.abs();
            let (mut w_sum, mut x_sum, mut y_sum, mut xy_sum, mut x2_sum) =
                (0.0, 0.0, 0.0, 0.0, 0.0);
            for k in left..=right {
                let (xk, yk) = (at(&xv, k), at(&yv, k));
                let t = (dx - xk).abs() * denom;
                let t = 1.0 - t * t * t;
                let w = t * t * t * at(&robust, k);
                let xkw = xk * w;
                w_sum += w;
                x_sum += xkw;
                y_sum += yk * w;
                xy_sum += yk * xkw;
                x2_sum += xk * xkw;
            }
            let (mx, my) = (x_sum / w_sum, y_sum / w_sum);
            let (mxy, mx2) = (xy_sum / w_sum, x2_sum / w_sum);
            let delta = mx2 - mx * mx;
            let slope = if delta.abs() < 1e-24 {
                0.0
            } else {
                (mxy - mx * my) / delta
            };
            fit[i] = my - slope * mx + slope * dx;
            residuals[i] = (yv[i] - fit[i]).abs();

            // Slide the window right while its next point is no farther than
            // its first.
            let next = i + 1;
            let mut far = right + 1;
            if far < n && next < n {
                let value = xv[next];
                while next > left && far < n && xv[far] - value <= value - xv[left] {
                    left += 1;
                    right = far;
                    far += 1;
                }
            }
        }
        if iteration == ITERATIONS {
            break;
        }
        let mut sorted: Vec<f64> = residuals.iter().copied().filter(|r| !r.is_nan()).collect();
        sorted.sort_by(f64::total_cmp);
        let median = array::quantile_sorted(&sorted, 0.5);
        if median.abs() < EPSILON {
            break;
        }
        for (weight, residual) in robust.iter_mut().zip(&residuals) {
            let arg = residual / (6.0 * median);
            *weight = if arg >= 1.0 {
                EPSILON
            } else {
                let w = 1.0 - arg * arg;
                w * w
            };
        }
    }

    let mut out: Vec<[f64; 2]> = Vec::new();
    let mut repeats = 0.0;
    for (x, fit) in xv.iter().zip(&fit) {
        let x = x + ux;
        match out.last_mut() {
            Some(last) if last[0] == x => {
                repeats += 1.0;
                last[1] += (fit - last[1]) / repeats;
            }
            last => {
                if let Some(last) = last {
                    last[1] += uy;
                }
                repeats = 0.0;
                out.push([x, *fit]);
            }
        }
    }
    if let Some(last) = out.last_mut() {
        last[1] += uy;
    }
    out
}
