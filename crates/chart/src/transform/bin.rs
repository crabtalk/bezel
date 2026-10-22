//! Vega-Lite's `bin` transform: Vega's `bin` over Vega-Lite's defaults.

use crate::data::Data;
use gpui::SharedString;
use js::math;

/// Vega's epsilon against floating point error at a bin's edge.
const EPSILON: f64 = 1e-14;

/// Equal-width bins: `start + k * step` up to `stop`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bins {
    pub start: f64,
    pub stop: f64,
    pub step: f64,
}

impl Bins {
    /// Vega's bin boundaries over the domain `lo..=hi`: a `NaN` start or
    /// stop is the domain's, and one outside it moves in to a step.
    pub fn edges(&self, lo: f64, hi: f64) -> Vec<f64> {
        let step = self.step;
        let mut start = if self.start.is_nan() { lo } else { self.start };
        let mut stop = if self.stop.is_nan() { hi } else { self.stop };
        if start < lo {
            start = step * (lo / step).ceil();
        }
        if stop > hi {
            stop = step * (hi / step).floor();
        }
        let count = ((stop + step / 2.0 - start) / step).ceil();
        match count > 0.0 && count.is_finite() {
            true => (0..count as usize)
                .map(|i| start + i as f64 * step)
                .collect(),
            false => Vec::new(),
        }
    }
}

/// Each row's bin of `field`: where it starts and where it ends, as
/// `output`. A missing value has neither, and a value outside the bins
/// starts and ends at an infinity.
#[derive(Clone, Debug, PartialEq)]
pub struct Bin {
    pub field: SharedString,
    /// The most bins. 0 reads as 20.
    pub maxbins: f64,
    /// The base steps are powers of.
    pub base: f64,
    /// What a step may be divided by while the bins stay within `maxbins`.
    pub divide: Vec<f64>,
    /// The values binned. `None` takes the field's least and greatest.
    pub extent: Option<[f64; 2]>,
    /// The span steps are chosen for. `None` takes the extent's.
    pub span: Option<f64>,
    /// An exact step.
    pub step: Option<f64>,
    /// The steps to choose from.
    pub steps: Option<Vec<f64>>,
    pub minstep: f64,
    /// Whether the bins start and stop on a step.
    pub nice: bool,
    /// A value the bins' edges are shifted to fall on.
    pub anchor: Option<f64>,
    pub output: [SharedString; 2],
}

impl Bin {
    /// Vega-Lite's defaults: at most 10 bins, output as `bin0` and `bin1`.
    pub fn new(field: impl Into<SharedString>) -> Self {
        Self {
            field: field.into(),
            maxbins: 10.0,
            base: 10.0,
            divide: vec![5.0, 2.0],
            extent: None,
            span: None,
            step: None,
            steps: None,
            minstep: 0.0,
            nice: true,
            anchor: None,
            output: ["bin0".into(), "bin1".into()],
        }
    }

    /// The bins `data`'s values of `field` fall in. A field with no number
    /// has `NaN` bins.
    pub fn bins(&self, data: &Data) -> Bins {
        let [min, max] = self.extent.unwrap_or_else(|| {
            let (lo, hi) = data
                .numbers(&self.field)
                .into_iter()
                .filter(|v| !v.is_nan())
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(v), hi.max(v))
                });
            match lo.is_finite() && hi.is_finite() {
                true => [lo, hi],
                false => [f64::NAN, f64::NAN],
            }
        });
        let Bins { start, stop, step } = self.nice_bins(min, max);
        let mut start = start;
        let mut stop = start + ((stop - start) / step).ceil() * step;
        if let Some(anchor) = self.anchor {
            let shift = anchor - (start + step * ((anchor - start) / step).floor());
            start += shift;
            stop += shift;
        }
        Bins { start, stop, step }
    }

    /// vega-statistics' `bin`.
    fn nice_bins(&self, mut min: f64, mut max: f64) -> Bins {
        let truthy = |v: f64| v != 0.0 && !v.is_nan();
        let maxbins = if truthy(self.maxbins) {
            self.maxbins
        } else {
            20.0
        };
        let base = if truthy(self.base) { self.base } else { 10.0 };
        let logb = base.ln();
        let span = [self.span.unwrap_or(0.0), max - min, min.abs()]
            .into_iter()
            .find(|v| truthy(*v))
            .unwrap_or(1.0);
        let minstep = if self.minstep.is_nan() {
            0.0
        } else {
            self.minstep
        };
        let step = match (self.step.filter(|s| truthy(*s)), &self.steps) {
            (Some(step), _) => step,
            (None, Some(steps)) => {
                let target = span / maxbins;
                let i = steps.iter().take_while(|s| **s < target).count();
                steps.get(i.saturating_sub(1)).copied().unwrap_or(f64::NAN)
            }
            (None, None) => {
                let level = (maxbins.ln() / logb).ceil();
                let mut step = minstep.max(math::pow(base, math::round(span.ln() / logb) - level));
                while (span / step).ceil() > maxbins {
                    step *= base;
                }
                for div in &self.divide {
                    let v = step / div;
                    if v >= minstep && span / v <= maxbins {
                        step = v;
                    }
                }
                step
            }
        };
        let v = step.ln();
        let precision = if v >= 0.0 {
            0.0
        } else {
            (-v / logb).trunc() + 1.0
        };
        let eps = math::pow(base, -precision - 1.0);
        if self.nice {
            let v = (min / step + eps).floor() * step;
            min = if min < v { v - step } else { v };
            max = (max / step).ceil() * step;
        }
        Bins {
            start: min,
            stop: if max == min { min + step } else { max },
            step,
        }
    }

    pub fn apply(&self, data: &Data) -> Data {
        let Bins { start, stop, step } = self.bins(data);
        let values = data.numbers(&self.field);
        let starts: Vec<f64> = values
            .iter()
            .map(|&v| match v {
                v if v.is_nan() => f64::NAN,
                v if v < start => f64::NEG_INFINITY,
                v if v > stop => f64::INFINITY,
                v => {
                    let v = math::max(start, math::min(v, stop - step));
                    start + step * (EPSILON + (v - start) / step).floor()
                }
            })
            .collect();
        let ends: Vec<f64> = starts
            .iter()
            .map(|&v| start + step * (1.0 + (v - start) / step))
            .collect();
        let [low, high] = self.output.clone();
        data.clone().number(low, starts).number(high, ends)
    }
}
