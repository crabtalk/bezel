//! vega-statistics' `bin`.

use d3::{format::number, math};

/// What chooses the bins.
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    /// The most bins. 0 reads as 20.
    pub maxbins: f64,
    /// The base steps are powers of. 0 reads as 10.
    pub base: f64,
    /// What a step may be divided by while the bins stay within `maxbins`.
    pub divide: Vec<f64>,
    /// The least and greatest values binned.
    pub extent: [f64; 2],
    /// The span steps are chosen for; 0 takes the extent's.
    pub span: f64,
    /// An exact step; 0 chooses one.
    pub step: f64,
    /// The steps to choose from.
    pub steps: Option<Vec<f64>>,
    pub minstep: f64,
    /// Whether the bins start and stop on a step.
    pub nice: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            maxbins: 20.0,
            base: 10.0,
            divide: vec![5.0, 2.0],
            extent: [f64::NAN, f64::NAN],
            span: 0.0,
            step: 0.0,
            steps: None,
            minstep: 0.0,
            nice: true,
        }
    }
}

/// Equal-width bins: `start + k * step` up to `stop`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bins {
    pub start: f64,
    pub stop: f64,
    pub step: f64,
}

/// `bin(params)`.
pub fn bin(params: &Params) -> Bins {
    let truthy = |v: f64| v != 0.0 && !v.is_nan();
    let maxbins = if truthy(params.maxbins) {
        params.maxbins
    } else {
        20.0
    };
    let base = if truthy(params.base) {
        params.base
    } else {
        10.0
    };
    let logb = math::log(base);
    let [mut min, mut max] = params.extent;
    let span = [params.span, max - min, min.abs()]
        .into_iter()
        .find(|v| truthy(*v))
        .unwrap_or(1.0);
    let minstep = if truthy(params.minstep) {
        params.minstep
    } else {
        0.0
    };
    let step = match (truthy(params.step), &params.steps) {
        (true, _) => params.step,
        (false, Some(steps)) => {
            let target = span / maxbins;
            let i = steps.iter().take_while(|s| **s < target).count();
            steps.get(i.saturating_sub(1)).copied().unwrap_or(f64::NAN)
        }
        (false, None) => {
            let level = (math::log(maxbins) / logb).ceil();
            let mut step = minstep.max(number::pow(
                base,
                number::round(math::log(span) / logb) - level,
            ));
            while (span / step).ceil() > maxbins {
                step *= base;
            }
            for div in &params.divide {
                let v = step / div;
                if v >= minstep && span / v <= maxbins {
                    step = v;
                }
            }
            step
        }
    };
    let v = math::log(step);
    let precision = if v >= 0.0 {
        0.0
    } else {
        (-v / logb).trunc() + 1.0
    };
    let eps = number::pow(base, -precision - 1.0);
    if params.nice {
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
