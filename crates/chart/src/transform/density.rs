//! Vega-Lite's `density` transform: Gaussian kernel density estimation.

use crate::{data::Data, transform::Groups};
use gpui::SharedString;
use vega::{
    statistics::{curve, distribution::Distribution},
    value::Value,
};

/// A smoothed density of `field`'s values per group, sampled along their
/// extent: one row per sample.
#[derive(Clone, Debug, PartialEq)]
pub struct Density {
    pub field: SharedString,
    pub groupby: Vec<SharedString>,
    /// The cumulative distribution in place of the density.
    pub cumulative: bool,
    /// Densities scaled by the group's row count.
    pub counts: bool,
    /// The kernel's bandwidth. `None` estimates one per group by Scott's rule.
    pub bandwidth: Option<f64>,
    /// Where to sample. `None` takes the values' extent.
    pub extent: Option<[f64; 2]>,
    /// An exact sample count, overriding `minsteps` and `maxsteps`.
    pub steps: Option<usize>,
    pub minsteps: usize,
    pub maxsteps: usize,
    pub resolve: Resolve,
    /// The sample's and the density's fields, Vega-Lite's `as`.
    pub output: [SharedString; 2],
}

/// How the groups' densities share their samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolve {
    /// Every group sampled at the same `maxsteps + 1` even points across all
    /// groups' extent.
    Shared,
    /// Each group over its own extent, adaptively between `minsteps` and
    /// `maxsteps`.
    Independent,
}

impl Density {
    /// Vega-Lite's defaults: shared samples, 25 to 200 steps, output as
    /// `value` and `density`.
    pub fn new(field: impl Into<SharedString>) -> Self {
        Self {
            field: field.into(),
            groupby: Vec::new(),
            cumulative: false,
            counts: false,
            bandwidth: None,
            extent: None,
            steps: None,
            minsteps: 25,
            maxsteps: 200,
            resolve: Resolve::Shared,
            output: ["value".into(), "density".into()],
        }
    }

    pub fn apply(&self, data: &Data) -> Data {
        let values = data.numbers(&self.field);
        let groups = Groups::of(data, &self.groupby);
        let or = |steps: usize, default: usize| if steps == 0 { default } else { steps };
        let steps = self.steps.filter(|steps| *steps > 0);
        let mut domain = self.extent;
        let mut minsteps = steps.unwrap_or(or(self.minsteps, 25));
        let mut maxsteps = steps.unwrap_or(or(self.maxsteps, 200));
        if self.resolve == Resolve::Shared {
            domain = domain.or_else(|| curve::extent(values.iter().copied()));
            minsteps = steps.unwrap_or(maxsteps);
            maxsteps = minsteps;
        }
        let points: Vec<Vec<[f64; 2]>> = groups
            .rows
            .iter()
            .map(|rows| {
                let support: Vec<f64> = rows
                    .iter()
                    .map(|row| values[*row])
                    .filter(|v| !v.is_nan())
                    .collect();
                let Some(local) = domain.or_else(|| curve::extent(support.iter().copied())) else {
                    return Vec::new();
                };
                let support: Vec<Value> = support.into_iter().map(Value::Number).collect();
                let kde = Distribution::kde(&support, self.bandwidth.unwrap_or(0.0));
                let scale = if self.counts { rows.len() as f64 } else { 1.0 };
                let f = |x| match self.cumulative {
                    true => kde.cdf(x),
                    false => kde.pdf(x),
                };
                curve::sample(f, local, minsteps, maxsteps)
                    .into_iter()
                    .map(|[x, y]| [x, y * scale])
                    .collect()
            })
            .collect();
        groups.table(data, &points, &self.output)
    }
}
