//! A channel's scale as Vega-Lite writes it: its type and properties, each
//! `None` for Vega-Lite's default for the channel, field type and mark.

use crate::{
    color,
    model::{Position, Shape},
    transform::Bins,
};
use d3::{color::Rgb, interpolate, scheme, time};
use gpui::{Hsla, SharedString};
use std::{str::FromStr, sync::Arc};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scale {
    pub kind: Option<ScaleType>,
    /// Whether a continuous domain takes in zero.
    pub zero: Option<bool>,
    /// Fixed instead of the data's. Marks outside a continuous one are drawn
    /// past the plot unless their layer has `clip`.
    pub domain: Option<Domain>,
    /// Over the domain's first end, and its last, after zero.
    pub domain_min: Option<f64>,
    pub domain_max: Option<f64>,
    /// The value a diverging colour scale centres on.
    pub domain_mid: Option<f64>,
    pub range: Option<Range>,
    /// Over the ends of a default range of two numbers.
    pub range_min: Option<f64>,
    pub range_max: Option<f64>,
    /// A colour scale's colours, over `range`.
    pub scheme: Option<Scheme>,
    pub reverse: Option<bool>,
    /// Whether values outside the domain map to the range's ends.
    pub clamp: bool,
    pub nice: Option<Nice>,
    /// A band or point scale's outer padding, and a band scale's inner; on
    /// x or y continuous, pixels the domain widens to leave at each end.
    pub padding: Option<f64>,
    pub padding_inner: Option<f64>,
    pub padding_outer: Option<f64>,
    /// Whether positions round to whole pixels.
    pub round: bool,
    /// How a continuous colour scale runs between its range's colours.
    pub interpolate: Option<interpolate::Interpolate>,
    /// A pow scale's exponent.
    pub exponent: Option<f64>,
    /// A log scale's base.
    pub base: Option<f64>,
    /// A symlog scale's slope about zero.
    pub constant: Option<f64>,
    /// On a quantitative position, bins whose edges are the axis's ticks. A
    /// `NaN` start or stop is the domain's.
    pub bins: Option<Bins>,
}

/// Vega-Lite's scale types, by its names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleType {
    Linear,
    Log,
    Pow,
    Sqrt,
    Symlog,
    Time,
    Utc,
    Ordinal,
    Band,
    Point,
    Quantile,
    Quantize,
    Threshold,
    BinOrdinal,
    /// No scale: each value is what the channel takes. Vega-Lite's
    /// `scale: null`.
    Identity,
}

impl FromStr for ScaleType {
    type Err = ();

    fn from_str(name: &str) -> Result<Self, ()> {
        Ok(match name {
            "linear" => Self::Linear,
            "log" => Self::Log,
            "pow" => Self::Pow,
            "sqrt" => Self::Sqrt,
            "symlog" => Self::Symlog,
            "time" => Self::Time,
            "utc" => Self::Utc,
            "ordinal" => Self::Ordinal,
            "band" => Self::Band,
            "point" => Self::Point,
            "quantile" => Self::Quantile,
            "quantize" => Self::Quantize,
            "threshold" => Self::Threshold,
            "bin-ordinal" => Self::BinOrdinal,
            _ => return Err(()),
        })
    }
}

impl ScaleType {
    /// Linear, log, pow, sqrt, symlog, time and utc.
    pub fn is_continuous(self) -> bool {
        matches!(
            self,
            Self::Linear
                | Self::Log
                | Self::Pow
                | Self::Sqrt
                | Self::Symlog
                | Self::Time
                | Self::Utc
        )
    }

    /// Ordinal, band and point.
    pub fn is_discrete(self) -> bool {
        matches!(self, Self::Ordinal | Self::Band | Self::Point)
    }

    /// Quantile, quantize, threshold and bin-ordinal: a continuous domain
    /// onto a discrete range.
    pub fn is_discretizing(self) -> bool {
        matches!(
            self,
            Self::Quantile | Self::Quantize | Self::Threshold | Self::BinOrdinal
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Domain {
    /// A continuous scale's ends, a piecewise one's stops, a threshold
    /// scale's thresholds. Times in milliseconds since the epoch.
    Numbers(Vec<f64>),
    /// A discrete scale's values in order, a number as its read-out text.
    /// Rows with other values are not drawn.
    Values(Vec<SharedString>),
    /// A continuous scale's data: the least and greatest of the values each
    /// row aggregates, the fields that hold them, in place of the channel's
    /// own.
    Unaggregated([SharedString; 2]),
    /// A continuous scale's data: these values in place of its rows', as
    /// Vega-Lite takes a domain from a data source of its own.
    Data(Arc<[f64]>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Range {
    /// For a numeric channel. On theta, radians.
    Numbers(Vec<f64>),
    /// For x or y: from the plot's left edge, or its top.
    Positions(Vec<Position>),
    Colors(Vec<Hsla>),
    Shapes(Vec<Shape>),
    Dashes(Vec<Vec<f32>>),
    Named(NamedRange),
    /// Each discrete value takes this field's value in its rows: one to
    /// one with the channel's field.
    Field(SharedString),
}

/// Vega's named ranges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NamedRange {
    Category,
    Ordinal,
    Heatmap,
    Ramp,
    Diverging,
    Symbol,
}

impl FromStr for NamedRange {
    type Err = ();

    fn from_str(name: &str) -> Result<Self, ()> {
        Ok(match name {
            "category" => Self::Category,
            "ordinal" => Self::Ordinal,
            "heatmap" => Self::Heatmap,
            "ramp" => Self::Ramp,
            "diverging" => Self::Diverging,
            "symbol" => Self::Symbol,
            _ => return Err(()),
        })
    }
}

/// A colour scheme, the part of it a scale takes, and how many colours a
/// discrete scale takes from it.
#[derive(Clone, Debug, PartialEq)]
pub struct Scheme {
    pub palette: Palette,
    /// `[0, 1]` for the whole of a ramp.
    pub extent: Option<[f64; 2]>,
    pub count: Option<usize>,
}

/// A scheme's colours.
#[derive(Clone, Debug, PartialEq)]
pub enum Palette {
    Named(scheme::Scheme),
    /// Spread evenly across `0..=1`, the scale's `interpolate` between.
    Colors(Vec<Hsla>),
}

impl From<scheme::Scheme> for Scheme {
    fn from(palette: scheme::Scheme) -> Self {
        Self {
            palette: Palette::Named(palette),
            extent: None,
            count: None,
        }
    }
}

impl Scheme {
    /// Whether it is a ramp, which a scale interpolates, rather than a list.
    pub fn is_continuous(&self) -> bool {
        match &self.palette {
            Palette::Named(named) => named.is_continuous(),
            Palette::Colors(_) => true,
        }
    }

    /// A ramp's colour at `t` in `0..=1` through its extent, which
    /// `reverse` turns round; a list of colours runs through `interpolate`.
    pub fn at(&self, t: f64, reverse: bool, interpolate: interpolate::Interpolate) -> Rgb {
        match &self.palette {
            Palette::Named(named) => named.at(t, self.extent, reverse),
            Palette::Colors(colors) => {
                let [start, end] = self.extent.unwrap_or([0.0, 1.0]);
                let (start, end) = match reverse {
                    true => (end, start),
                    false => (start, end),
                };
                let colors: Vec<Rgb> = colors.iter().copied().map(color::rgb).collect();
                interpolate.piecewise(&colors, start + t * (end - start))
            }
        }
    }

    /// The colours a discrete scale of `count` values takes, as
    /// [`scheme::Scheme::discrete`] picks them.
    pub fn discrete(
        &self,
        count: usize,
        ordinal: bool,
        interpolate: interpolate::Interpolate,
    ) -> Vec<Rgb> {
        match &self.palette {
            Palette::Named(named) => named.discrete(count, self.extent, ordinal),
            Palette::Colors(_) => (1..=count)
                .map(|i| self.at(i as f64 / (count + 1) as f64, false, interpolate))
                .collect(),
        }
    }

    /// A list's colours as listed; a ramp's evenly sampled `count` times.
    pub fn colors(&self, count: usize, interpolate: interpolate::Interpolate) -> Vec<Rgb> {
        match &self.palette {
            Palette::Named(named) if !named.is_continuous() => {
                named.colors().into_iter().take(count).collect()
            }
            _ => self.discrete(count, false, interpolate),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Nice {
    Off,
    /// Ends on the ticks of about this many.
    Ticks(f64),
    /// A time scale's ends on every `step`th boundary of `unit`.
    Interval {
        unit: time::Unit,
        step: f64,
    },
}

/// How a position channel's values stack, Vega's stack offsets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stack {
    /// A bar's or an area's stack from zero, nothing else's.
    #[default]
    Auto,
    Zero,
    /// Each stack's parts as shares of one.
    Normalize,
    /// Each stack centred on the middle of the largest.
    Center,
    /// Each from zero, overlapping.
    Off,
}
