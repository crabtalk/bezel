//! vega-statistics: the estimators and generators Vega's transforms use.

use crate::value::Value;
use d3::{array, math};

pub mod bin;
pub mod curve;
pub mod distribution;
pub mod dotbin;
pub mod regression;

/// The seed a view's [`Random`] starts from: the `randomLCG(123456789)`
/// Vega-Lite's compiled examples set as Vega's `random`.
const SEED: f64 = 123_456_789.0;

/// vega-statistics' `random`, set to `randomLCG`: numbers in `[0, 1)`,
/// without end. It also keeps the second number of `sampleNormal`'s last
/// Box-Muller pair.
#[derive(Clone, Debug, PartialEq)]
pub struct Random {
    seed: f64,
    spare: f64,
}

impl Default for Random {
    fn default() -> Self {
        Self {
            seed: SEED,
            spare: f64::NAN,
        }
    }
}

impl Iterator for Random {
    type Item = f64;

    fn next(&mut self) -> Option<f64> {
        self.seed = (1_103_515_245.0 * self.seed + 12_345.0) % 2_147_483_647.0;
        Some(self.seed / 2_147_483_647.0)
    }
}

impl Random {
    /// The next number in `[0, 1)`.
    pub fn draw(&mut self) -> f64 {
        self.next().expect("endless")
    }

    /// `sampleNormal(mean, stdev)`: by the Box-Muller transform, a pair at
    /// a time.
    pub fn normal(&mut self, mean: f64, stdev: f64) -> f64 {
        let x = if !self.spare.is_nan() {
            std::mem::replace(&mut self.spare, f64::NAN)
        } else {
            let (mut x, mut y, mut rds);
            loop {
                x = self.draw() * 2.0 - 1.0;
                y = self.draw() * 2.0 - 1.0;
                rds = x * x + y * y;
                if rds != 0.0 && rds <= 1.0 {
                    break;
                }
            }
            let c = (-2.0 * math::log(rds) / rds).sqrt();
            self.spare = y * c;
            x * c
        };
        mean + x * stdev
    }
}

/// `numbers`: each value but null, undefined and `""` as `+value`, those
/// that are `NaN` left out.
pub fn numbers<'a>(values: impl IntoIterator<Item = &'a Value>) -> Vec<f64> {
    values
        .into_iter()
        .filter(|v| !v.is_nullish() && !matches!(v, Value::String(s) if s.is_empty()))
        .map(Value::to_number)
        .filter(|v| !v.is_nan())
        .collect()
}

/// `quantiles(values, p)`: each `p`-quantile of the [`numbers`],
/// `undefined` where there are none.
pub fn quantiles(values: &[f64], p: &[f64]) -> Vec<Value> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    p.iter().map(|&p| quantile_sorted(&sorted, p)).collect()
}

/// `quartiles(values)`.
pub fn quartiles(values: &[f64]) -> [Value; 3] {
    let [q1, q2, q3]: [Value; 3] = quantiles(values, &[0.25, 0.5, 0.75])
        .try_into()
        .expect("three");
    [q1, q2, q3]
}

/// d3-array's `quantileSorted`: `undefined` for no values.
fn quantile_sorted(sorted: &[f64], p: f64) -> Value {
    if sorted.is_empty() || p.is_nan() {
        Value::Undefined
    } else {
        Value::Number(array::quantile_sorted(sorted, p))
    }
}

/// `bootstrapCI(values, samples, alpha)`: the `alpha` confidence interval of
/// the mean of `values`, from `samples` resamples drawn from `random`.
/// `[undefined, undefined]` where `count`, the values before [`numbers`]
/// dropped any, is 0.
pub fn bootstrap_ci(
    values: &[f64],
    count: usize,
    samples: usize,
    alpha: f64,
    random: &mut dyn FnMut() -> f64,
) -> [Value; 2] {
    if count == 0 {
        return [Value::Undefined, Value::Undefined];
    }
    let n = values.len();
    let mut means: Vec<f64> = (0..samples)
        .map(|_| {
            let mut a = 0.0;
            for _ in 0..n {
                a += values[(random() * n as f64) as usize];
            }
            a / n as f64
        })
        .collect();
    // d3-array's `quantile` reads only the numbers.
    means.retain(|v| !v.is_nan());
    means.sort_by(f64::total_cmp);
    [
        quantile_sorted(&means, alpha / 2.0),
        quantile_sorted(&means, 1.0 - alpha / 2.0),
    ]
}
