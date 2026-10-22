//! Vega-Lite's `quantile` transform.

use crate::{data::Data, transform::Groups};
use gpui::SharedString;
use js::array;

/// Quantiles of `field`'s values per group: one row per probability.
#[derive(Clone, Debug, PartialEq)]
pub struct Quantile {
    pub field: SharedString,
    pub groupby: Vec<SharedString>,
    /// The probabilities, in `(0, 1)`. `None` takes every `step` from half a
    /// step up to 1.
    pub probs: Option<Vec<f64>>,
    /// 0 reads as 0.01.
    pub step: f64,
    /// The probability's and the quantile's fields, Vega-Lite's `as`.
    pub output: [SharedString; 2],
}

impl Quantile {
    /// Vega-Lite's defaults: a step of 0.01, output as `prob` and `value`.
    pub fn new(field: impl Into<SharedString>) -> Self {
        Self {
            field: field.into(),
            groupby: Vec::new(),
            probs: None,
            step: 0.01,
            output: ["prob".into(), "value".into()],
        }
    }

    pub fn apply(&self, data: &Data) -> Data {
        let probs = self.probs.clone().unwrap_or_else(|| {
            let step = if self.step == 0.0 || self.step.is_nan() {
                0.01
            } else {
                self.step
            };
            let start = step / 2.0;
            let count = ((1.0 - 1e-14 - start) / step).ceil().max(0.0) as usize;
            (0..count).map(|i| start + i as f64 * step).collect()
        });
        let values = data.numbers(&self.field);
        let groups = Groups::of(data, &self.groupby);
        let points: Vec<Vec<[f64; 2]>> = groups
            .rows
            .iter()
            .map(|rows| {
                let mut sorted: Vec<f64> = rows
                    .iter()
                    .map(|row| values[*row])
                    .filter(|v| !v.is_nan())
                    .collect();
                sorted.sort_by(f64::total_cmp);
                probs
                    .iter()
                    .map(|p| [*p, array::quantile_sorted(&sorted, *p)])
                    .collect()
            })
            .collect();
        groups.table(data, &points, &self.output)
    }
}
