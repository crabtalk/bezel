//! Vega-Lite's `density` transform: Gaussian kernel density estimation.

use crate::{
    data::Data,
    transform::{Groups, curve},
};
use d3::array;
use gpui::SharedString;
use std::f64::consts::PI;

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
                let kde = Kde::new(support, self.bandwidth);
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

/// A Gaussian kernel density over `support`.
struct Kde {
    support: Vec<f64>,
    bandwidth: f64,
}

impl Kde {
    /// A bandwidth of `None`, 0 or `NaN` is estimated.
    fn new(support: Vec<f64>, bandwidth: Option<f64>) -> Self {
        let bandwidth = bandwidth
            .filter(|b| *b != 0.0 && !b.is_nan())
            .unwrap_or_else(|| Self::scott(&support));
        Self { support, bandwidth }
    }

    /// Scott's rule: `1.06 · min(σ, IQR / 1.34) · n^-0.2`, falling back to σ,
    /// then the first quartile's magnitude, then 1, where one is 0 or
    /// undefined.
    fn scott(support: &[f64]) -> f64 {
        let n = support.len();
        // Welford's sample variance, as d3's `deviation`.
        let (mut count, mut mean, mut sum) = (0.0, 0.0, 0.0);
        for value in support {
            let delta = value - mean;
            count += 1.0;
            mean += delta / count;
            sum += delta * (value - mean);
        }
        let deviation = if count > 1.0 {
            (sum / (count - 1.0)).sqrt()
        } else {
            f64::NAN
        };
        let mut sorted = support.to_vec();
        sorted.sort_by(f64::total_cmp);
        let q1 = array::quantile_sorted(&sorted, 0.25);
        let q3 = array::quantile_sorted(&sorted, 0.75);
        let iqr = (q3 - q1) / 1.34;
        let truthy = |v: f64| v != 0.0 && !v.is_nan();
        let min = if deviation.is_nan() || iqr.is_nan() {
            f64::NAN
        } else {
            deviation.min(iqr)
        };
        let v = [min, deviation, q1.abs()]
            .into_iter()
            .find(|v| truthy(*v))
            .unwrap_or(1.0);
        1.06 * v * (n as f64).powf(-0.2)
    }

    fn pdf(&self, x: f64) -> f64 {
        let y: f64 = self
            .support
            .iter()
            .map(|s| {
                let z = (x - s) / self.bandwidth;
                (-0.5 * z * z).exp() / (2.0 * PI).sqrt()
            })
            .sum();
        y / self.bandwidth / self.support.len() as f64
    }

    fn cdf(&self, x: f64) -> f64 {
        let y: f64 = self
            .support
            .iter()
            .map(|s| normal_cdf((x - s) / self.bandwidth))
            .sum();
        y / self.support.len() as f64
    }
}

/// The standard normal distribution's cumulative probability, by West's
/// (2009) approximation.
fn normal_cdf(z: f64) -> f64 {
    let abs = z.abs();
    let cd = if abs > 37.0 {
        0.0
    } else {
        let exp = (-abs * abs / 2.0).exp();
        if abs < 7.07106781186547 {
            let mut sum = 3.52624965998911e-02 * abs + 0.700383064443688;
            sum = sum * abs + 6.37396220353165;
            sum = sum * abs + 33.912866078383;
            sum = sum * abs + 112.079291497871;
            sum = sum * abs + 221.213596169931;
            sum = sum * abs + 220.206867912376;
            let cd = exp * sum;
            let mut sum = 8.83883476483184e-02 * abs + 1.75566716318264;
            sum = sum * abs + 16.064177579207;
            sum = sum * abs + 86.7807322029461;
            sum = sum * abs + 296.564248779674;
            sum = sum * abs + 637.333633378831;
            sum = sum * abs + 793.826512519948;
            sum = sum * abs + 440.413735824752;
            cd / sum
        } else {
            let mut sum = abs + 0.65;
            sum = abs + 4.0 / sum;
            sum = abs + 3.0 / sum;
            sum = abs + 2.0 / sum;
            sum = abs + 1.0 / sum;
            exp / sum / 2.506628274631
        }
    };
    if z > 0.0 { 1.0 - cd } else { cd }
}
