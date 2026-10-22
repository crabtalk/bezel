//! Vega-Lite's `loess` transform: locally-weighted regression.

use crate::{data::Data, transform::Groups};
use gpui::SharedString;
use vega::statistics::regression;

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
                regression::loess(&pairs, bandwidth)
            })
            .collect();
        groups.table(data, &points, &self.output)
    }
}
