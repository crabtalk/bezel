//! Scales from a continuous domain onto a discrete range: d3's `quantile`,
//! `quantize` and `threshold`, and vega-scale's `bin-ordinal`. Each answers
//! an index into its range.

use crate::array;

/// d3's `scaleQuantile`: `count` groups of the domain's values, as many in
/// each.
#[derive(Clone, Debug, PartialEq)]
pub struct Quantile {
    /// The domain's values but `NaN`, sorted.
    pub domain: Vec<f64>,
    /// The `count - 1` quantiles between the groups.
    pub thresholds: Vec<f64>,
}

impl Quantile {
    /// Over `values`, which may hold `NaN`, into a range of `count`.
    pub fn new(values: &[f64], count: usize) -> Self {
        let mut sorted: Vec<f64> = values.iter().copied().filter(|v| !v.is_nan()).collect();
        sorted.sort_by(f64::total_cmp);
        let n = count.max(1);
        Self {
            thresholds: (1..n)
                .map(|i| array::quantile_sorted(&sorted, i as f64 / n as f64))
                .collect(),
            domain: sorted,
        }
    }

    pub fn index(&self, x: f64) -> Option<usize> {
        (!x.is_nan()).then(|| array::bisect_right(&self.thresholds, x, 0, self.thresholds.len()))
    }
}

/// d3's `scaleQuantize`: `domain` cut into `count` equal pieces.
#[derive(Clone, Debug, PartialEq)]
pub struct Quantize {
    pub domain: [f64; 2],
    /// The `count - 1` cuts between the pieces.
    pub thresholds: Vec<f64>,
}

impl Quantize {
    pub fn new([x0, x1]: [f64; 2], count: usize) -> Self {
        let n = count.max(1) - 1;
        Self {
            thresholds: (0..n)
                .map(|i| {
                    let (i, n) = (i as f64, n as f64);
                    ((i + 1.0) * x1 - (i - n) * x0) / (n + 1.0)
                })
                .collect(),
            domain: [x0, x1],
        }
    }

    pub fn index(&self, x: f64) -> Option<usize> {
        (!x.is_nan()).then(|| array::bisect_right(&self.thresholds, x, 0, self.thresholds.len()))
    }
}

/// d3's `scaleThreshold`: index `i` below `domain[i]`, the last at or past
/// the last threshold the range has room for.
#[derive(Clone, Debug, PartialEq)]
pub struct Threshold {
    pub domain: Vec<f64>,
    /// How many values the range holds.
    pub count: usize,
}

impl Threshold {
    pub fn index(&self, x: f64) -> Option<usize> {
        let n = self.domain.len().min(self.count.saturating_sub(1));
        (!x.is_nan()).then(|| array::bisect_right(&self.domain, x, 0, n))
    }
}

/// vega-scale's `scaleBinOrdinal`: the bin, of those `bins` bound, that `x`
/// falls in, round and round a range of `count`. `None` below the first
/// bound.
#[derive(Clone, Debug, PartialEq)]
pub struct BinOrdinal {
    pub bins: Vec<f64>,
    pub count: usize,
}

impl BinOrdinal {
    pub fn index(&self, x: f64) -> Option<usize> {
        if x.is_nan() || self.count == 0 {
            return None;
        }
        let bin = array::bisect_right(&self.bins, x, 0, self.bins.len()).checked_sub(1)?;
        Some(bin % self.count)
    }
}
