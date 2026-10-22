//! vega-scale's band and point scales, d3's with its paddings held in
//! `0..=1` and an empty domain spanning nothing.

use crate::format::number;

/// `count` bands across `range`, in domain order from `range[0]`, which may
/// be the greater end. A point scale is a band scale with an inner padding
/// of 1, whose bands have no width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub count: usize,
    pub range: [f64; 2],
    /// The share of a step between bands left empty.
    pub padding_inner: f64,
    /// Before the first band and after the last, in steps.
    pub padding_outer: f64,
    /// Where the bands sit in what the paddings leave, 0 at the start.
    pub align: f64,
    /// Whether the step, the start and the bandwidth are whole numbers.
    pub round: bool,
}

impl Band {
    /// vega-scale's `bandSpace`: how many steps a range of `count` bands
    /// holds.
    pub fn space(count: usize, padding_inner: f64, padding_outer: f64) -> f64 {
        let space = count as f64 - padding_inner + padding_outer * 2.0;
        match count {
            0 => 0.0,
            _ if space > 0.0 => space,
            _ => 1.0,
        }
    }

    /// No padding, centred, unrounded.
    pub fn new(count: usize, range: [f64; 2]) -> Self {
        Self {
            count,
            range,
            padding_inner: 0.0,
            padding_outer: 0.0,
            align: 0.5,
            round: false,
        }
    }

    /// d3's `scalePoint`, `padding` its outer padding.
    pub fn point(count: usize, range: [f64; 2], padding: f64) -> Self {
        Self {
            padding_inner: 1.0,
            padding_outer: padding,
            ..Self::new(count, range)
        }
    }

    /// The first band's start, the step and the bandwidth, from the range's
    /// lesser end.
    fn layout(&self) -> (f64, f64, f64) {
        let inner = self.padding_inner.clamp(0.0, 1.0);
        let outer = self.padding_outer.clamp(0.0, 1.0);
        let align = self.align.clamp(0.0, 1.0);
        let n = self.count as f64;
        let reverse = self.range[1] < self.range[0];
        let (mut start, stop) = match reverse {
            true => (self.range[1], self.range[0]),
            false => (self.range[0], self.range[1]),
        };
        let space = Self::space(self.count, inner, outer);
        let mut step = (stop - start) / if space == 0.0 { 1.0 } else { space };
        if self.round {
            step = step.floor();
        }
        start += (stop - start - step * (n - inner)) * align;
        let mut bandwidth = step * (1.0 - inner);
        if self.round {
            start = number::round(start);
            bandwidth = number::round(bandwidth);
        }
        (start, step, bandwidth)
    }

    pub fn step(&self) -> f64 {
        self.layout().1
    }

    pub fn bandwidth(&self) -> f64 {
        self.layout().2
    }

    /// Where band `index` starts: its lesser edge.
    pub fn at(&self, index: usize) -> f64 {
        let (start, step, _) = self.layout();
        let index = match self.range[1] < self.range[0] {
            true => self.count.saturating_sub(1).saturating_sub(index),
            false => index,
        };
        start + step * index as f64
    }

    /// The band `position` falls in, as Vega's `invert` reads it. `None`
    /// outside the range and between bands.
    pub fn invert(&self, position: f64) -> Option<usize> {
        if position.is_nan() || self.count == 0 {
            return None;
        }
        let (start, step, bandwidth) = self.layout();
        let stop = self.range[0].max(self.range[1]);
        let values: Vec<f64> = (0..self.count).map(|i| start + step * i as f64).collect();
        if position < values[0] || position > stop {
            return None;
        }
        let a = values.partition_point(|v| *v <= position).saturating_sub(1);
        if position - values[a] > bandwidth + 1e-10 {
            return None;
        }
        Some(match self.range[1] < self.range[0] {
            true => self.count - 1 - a,
            false => a,
        })
    }
}
