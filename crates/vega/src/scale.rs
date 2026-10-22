//! vega-scale's scales and vega-encode's `Scale` transform, which sets one
//! up from a scale definition: its domain, bins and range.

use crate::{Error, value::Value};
use d3::{
    color::Rgb,
    format::{NumberFormat, Specifier},
    interpolate::Interpolate,
    scale::{
        Band, BinOrdinal, Continuous, Diverging, Quantile, Quantize, Sequential, Threshold,
        Transform,
    },
    scheme::Scheme,
    time::{Interval, date::LocalZone},
};
use std::str::FromStr;

/// A scale's type, by Vega's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    Identity,
    Linear,
    Log,
    Pow,
    Sqrt,
    Symlog,
    Time,
    Utc,
    /// `sequential`, or `sequential-<kind>`.
    Sequential(Kind),
    /// `diverging-<kind>`.
    Diverging(Kind),
    Quantile,
    Quantize,
    Threshold,
    BinOrdinal,
    Ordinal,
    Band,
    Point,
}

/// The continuous kind under a sequential or diverging scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Linear,
    Log,
    Pow,
    Sqrt,
    Symlog,
}

impl FromStr for Type {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, String> {
        let kind = |name: &str| -> Option<Kind> {
            Some(match name {
                "linear" => Kind::Linear,
                "log" => Kind::Log,
                "pow" => Kind::Pow,
                "sqrt" => Kind::Sqrt,
                "symlog" => Kind::Symlog,
                _ => return None,
            })
        };
        Ok(match name {
            "identity" => Self::Identity,
            "linear" => Self::Linear,
            "log" => Self::Log,
            "pow" => Self::Pow,
            "sqrt" => Self::Sqrt,
            "symlog" => Self::Symlog,
            "time" => Self::Time,
            "utc" => Self::Utc,
            "sequential" => Self::Sequential(Kind::Linear),
            "quantile" => Self::Quantile,
            "quantize" => Self::Quantize,
            "threshold" => Self::Threshold,
            "bin-ordinal" => Self::BinOrdinal,
            "ordinal" => Self::Ordinal,
            "band" => Self::Band,
            "point" => Self::Point,
            other => match other.split_once('-') {
                Some(("sequential", k)) if kind(k).is_some() => {
                    Self::Sequential(kind(k).expect("checked"))
                }
                Some(("diverging", k)) if kind(k).is_some() => {
                    Self::Diverging(kind(k).expect("checked"))
                }
                _ => return Err(format!("Unrecognized scale type: \"{other}\"")),
            },
        })
    }
}

impl Type {
    /// The name vega-scale registers it by.
    pub fn name(self) -> String {
        let kind = |k: Kind| match k {
            Kind::Linear => "linear",
            Kind::Log => "log",
            Kind::Pow => "pow",
            Kind::Sqrt => "sqrt",
            Kind::Symlog => "symlog",
        };
        match self {
            Self::Identity => "identity".into(),
            Self::Linear => "linear".into(),
            Self::Log => "log".into(),
            Self::Pow => "pow".into(),
            Self::Sqrt => "sqrt".into(),
            Self::Symlog => "symlog".into(),
            Self::Time => "time".into(),
            Self::Utc => "utc".into(),
            Self::Sequential(k) => format!("sequential-{}", kind(k)),
            Self::Diverging(k) => format!("diverging-{}", kind(k)),
            Self::Quantile => "quantile".into(),
            Self::Quantize => "quantize".into(),
            Self::Threshold => "threshold".into(),
            Self::BinOrdinal => "bin-ordinal".into(),
            Self::Ordinal => "ordinal".into(),
            Self::Band => "band".into(),
            Self::Point => "point".into(),
        }
    }

    /// vega-scale's `isContinuous`.
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
                | Self::Sequential(_)
                | Self::Diverging(_)
        )
    }

    /// vega-scale's `isDiscrete`.
    pub fn is_discrete(self) -> bool {
        matches!(
            self,
            Self::BinOrdinal | Self::Ordinal | Self::Band | Self::Point
        )
    }

    /// vega-scale's `isDiscretizing`.
    pub fn is_discretizing(self) -> bool {
        matches!(
            self,
            Self::Quantile | Self::Quantize | Self::Threshold | Self::BinOrdinal
        )
    }

    /// vega-scale's `isInterpolating`.
    pub fn is_interpolating(self) -> bool {
        matches!(self, Self::Sequential(_) | Self::Diverging(_))
    }

    /// vega-scale's `isLogarithmic`.
    pub fn is_logarithmic(self) -> bool {
        matches!(
            self,
            Self::Log | Self::Sequential(Kind::Log) | Self::Diverging(Kind::Log)
        )
    }

    /// vega-scale's `isTemporal`.
    pub fn is_temporal(self) -> bool {
        matches!(self, Self::Time | Self::Utc)
    }

    /// vega-scale's `isQuantile`.
    pub fn is_quantile(self) -> bool {
        self == Self::Quantile
    }
}

/// How a continuous scale's interpolator runs through colours: from
/// `start` to `end` of `colors` spread across `0..=1`.
#[derive(Clone, Debug, PartialEq)]
pub struct Interpolator {
    colors: Vec<Rgb>,
    method: Interpolate,
    start: f64,
    end: f64,
}

impl Interpolator {
    /// The colour at `t`.
    pub fn at(&self, t: f64) -> Rgb {
        self.method
            .piecewise(&self.colors, self.start + t * (self.end - self.start))
    }
}

/// A scale ready to map values.
#[derive(Clone, Debug)]
pub struct Scale {
    pub type_: Type,
    map: Map,
    /// Bin boundaries, where the scale has them.
    pub bins: Option<Vec<f64>>,
}

#[derive(Clone, Debug)]
enum Map {
    Identity(Vec<Value>),
    /// A continuous scale onto numbers, or onto colours between `colors`.
    Continuous {
        scale: Continuous,
        colors: Option<(Vec<Rgb>, Interpolate)>,
    },
    Sequential(Sequential, Interpolator),
    Diverging(Diverging, Interpolator),
    Quantile(Quantile, Vec<Value>),
    Quantize(Quantize, Vec<Value>),
    Threshold(Threshold, Vec<Value>),
    BinOrdinal(BinOrdinal, Vec<Value>),
    Ordinal {
        domain: Vec<Value>,
        range: Vec<Value>,
        /// Whether a value outside the domain joins it.
        implicit: bool,
    },
    Band {
        domain: Vec<Value>,
        band: Band,
    },
}

/// A scheme as a scale definition gives it.
#[derive(Clone, Debug, PartialEq)]
pub enum SchemeParam {
    Named(String),
    Colors(Vec<Value>),
}

/// How a definition asks for a nice domain.
#[derive(Clone, Debug, PartialEq)]
pub enum NiceParam {
    /// `true`: d3's default.
    Default,
    Count(f64),
    /// A time unit, every `step`.
    Interval(String, f64),
}

/// What a scale definition gives the `Scale` transform, its signals and data
/// references resolved.
#[derive(Clone, Debug, Default)]
pub struct Params {
    pub domain: Option<Vec<Value>>,
    pub domain_raw: Option<Vec<Value>>,
    pub domain_min: Option<Value>,
    pub domain_max: Option<Value>,
    pub domain_mid: Option<Value>,
    pub domain_implicit: bool,
    pub range: Option<Vec<Value>>,
    pub range_step: Option<f64>,
    pub scheme: Option<SchemeParam>,
    pub scheme_extent: Option<[f64; 2]>,
    pub scheme_count: Option<f64>,
    pub interpolate: Option<String>,
    pub interpolate_gamma: Option<f64>,
    pub bins: Option<Vec<f64>>,
    /// `{step, start, stop}` bins, `NaN` for a missing bound.
    pub bins_step: Option<[f64; 3]>,
    pub nice: Option<NiceParam>,
    pub zero: Option<bool>,
    pub padding: Option<f64>,
    pub padding_inner: Option<f64>,
    pub padding_outer: Option<f64>,
    pub align: Option<f64>,
    pub round: bool,
    pub reverse: bool,
    pub clamp: bool,
    pub base: Option<f64>,
    pub exponent: Option<f64>,
    pub constant: Option<f64>,
}

const DEFAULT_COUNT: f64 = 5.0;

impl Scale {
    /// vega-encode's `Scale` transform for a `type_` scale.
    pub fn new(type_: Type, params: &Params) -> Result<Self, Error> {
        let type_ = key(type_, params);
        let numbers =
            |values: &[Value]| -> Vec<f64> { values.iter().map(Value::to_number).collect() };
        // `configureDomain`.
        let (domain, count): (Vec<Value>, usize) = match &params.domain_raw {
            Some(raw) => (raw.clone(), raw.len()),
            None => match &params.domain {
                None => (Vec::new(), 0),
                Some(domain) => {
                    let mut domain = domain.clone();
                    let zero = params.zero.unwrap_or(
                        params.bins.is_none()
                            && params.bins_step.is_none()
                            && matches!(type_, Type::Linear | Type::Pow | Type::Sqrt),
                    );
                    let bounded = params.domain_min.is_some()
                        || params.domain_max.is_some()
                        || params.domain_mid.is_some();
                    if (zero || bounded) && !domain.is_empty() {
                        let n = match domain.len() - 1 {
                            0 => 1,
                            n => n,
                        };
                        if zero {
                            if domain[0].to_number() > 0.0 {
                                domain[0] = Value::Number(0.0);
                            }
                            if domain.get(n).is_some_and(|d| d.to_number() < 0.0) {
                                domain[n] = Value::Number(0.0);
                            }
                        }
                        if let Some(min) = &params.domain_min {
                            domain[0] = min.clone();
                        }
                        if let Some(max) = &params.domain_max {
                            if n < domain.len() {
                                domain[n] = max.clone();
                            } else {
                                domain.push(max.clone());
                            }
                        }
                        if let Some(mid) = &params.domain_mid {
                            let m = mid.to_number();
                            let i = if m > domain[n.min(domain.len() - 1)].to_number() {
                                n + 1
                            } else if m < domain[0].to_number() {
                                0
                            } else {
                                n
                            };
                            domain.insert(i.min(domain.len()), mid.clone());
                        }
                    }
                    if type_.is_continuous()
                        && !type_.is_interpolating()
                        && params.padding.is_some_and(|p| p != 0.0)
                        && domain.first().map(Value::to_number)
                            != domain.last().map(Value::to_number)
                    {
                        let range = numbers(params.range.as_deref().unwrap_or_default());
                        domain = pad(type_, &numbers(&domain), &range, params)
                            .into_iter()
                            .map(Value::Number)
                            .collect();
                    }
                    let count = domain.len();
                    (domain, count)
                }
            },
        };
        let raw = params.domain_raw.is_some();
        let defined = params.domain.is_some() || raw;
        let mut count = count;

        // `configureBins`.
        let mut bins = params.bins.clone();
        if let Some([step, start, stop]) = params.bins_step {
            let (lo, hi) = (
                domain.first().map_or(f64::NAN, Value::to_number),
                domain.last().map_or(f64::NAN, Value::to_number),
            );
            let mut start = if start.is_nan() { lo } else { start };
            let mut stop = if stop.is_nan() { hi } else { stop };
            if step == 0.0 || step.is_nan() {
                return Err(Error::Spec(
                    "Scale bins parameter missing step property.".into(),
                ));
            }
            if start < lo {
                start = step * (lo / step).ceil();
            }
            if stop > hi {
                stop = step * (hi / step).floor();
            }
            bins = Some(d3_range(start, stop + step / 2.0, step));
        }
        let mut domain = domain;
        if type_ == Type::BinOrdinal {
            match &bins {
                None => bins = Some(numbers(&domain)),
                Some(b) if !defined => {
                    domain = b.iter().copied().map(Value::Number).collect();
                    count = b.len();
                }
                Some(_) => {}
            }
        }

        // `configureRange`.
        let mut range = params.range.clone();
        if let Some(step) = params.range_step {
            if !matches!(type_, Type::Band | Type::Point) {
                return Err(Error::Spec(
                    "Only band and point scales support rangeStep.".into(),
                ));
            }
            let outer = params.padding_outer.or(params.padding).unwrap_or(0.0);
            let inner = match type_ {
                Type::Point => 1.0,
                _ => params.padding_inner.or(params.padding).unwrap_or(0.0),
            };
            range = Some(vec![
                Value::Number(0.0),
                Value::Number(step * Band::space(count, inner, outer)),
            ]);
        }
        let method = || -> Result<Interpolate, Error> {
            let name = params.interpolate.as_deref().unwrap_or("rgb");
            let method: Interpolate = name
                .parse()
                .map_err(|_| Error::Unsupported(format!("interpolate {name}")))?;
            Ok(match params.interpolate_gamma {
                Some(gamma) => method.with_gamma(gamma),
                None => method,
            })
        };
        let mut interpolator: Option<Interpolator> = None;
        if params.range_step.is_none()
            && let Some(scheme) = &params.scheme
        {
            match scheme_range(type_, params, scheme, count, &method)? {
                SchemeRange::Interpolator(i) => interpolator = Some(i),
                SchemeRange::Colors(colors) => range = Some(colors),
            }
        }
        let flip = |mut values: Vec<Value>| {
            if params.reverse {
                values.reverse();
            }
            values
        };
        if interpolator.is_none()
            && type_.is_interpolating()
            && let Some(range) = &range
        {
            let colors = flip(range.clone())
                .iter()
                .map(|v| v.to_string().parse::<Rgb>().unwrap_or(nan_color()))
                .collect();
            interpolator = Some(Interpolator {
                colors,
                method: method()?,
                start: 0.0,
                end: 1.0,
            });
        }
        let range = range.map(flip);

        let map = match type_ {
            Type::Identity => Map::Identity(domain),
            Type::Linear
            | Type::Log
            | Type::Pow
            | Type::Sqrt
            | Type::Symlog
            | Type::Time
            | Type::Utc => {
                let transform = transform(type_, params);
                let mut scale = Continuous::new(transform, numbers(&domain), vec![0.0, 1.0]);
                scale.clamp = params.clamp;
                nice(&mut scale, type_, params, bins.as_deref())?;
                let mut colors = None;
                if let Some(range) = &range {
                    if range.iter().all(|v| matches!(v, Value::String(_))) && !range.is_empty() {
                        let parsed: Vec<Rgb> = range
                            .iter()
                            .map(|v| v.to_string().parse::<Rgb>().unwrap_or(nan_color()))
                            .collect();
                        let method = match params.interpolate {
                            Some(_) => method()?,
                            None => Interpolate::Rgb { gamma: 1.0 },
                        };
                        scale.range = (0..parsed.len()).map(|i| i as f64).collect();
                        colors = Some((parsed, method));
                    } else {
                        scale.range = numbers(range);
                        scale.round = params.round && params.interpolate.is_none();
                    }
                } else {
                    scale.round = params.round;
                }
                Map::Continuous { scale, colors }
            }
            Type::Sequential(kind) | Type::Diverging(kind) => {
                let transform = transform_kind(kind, params);
                let mut d = numbers(&domain);
                let interpolator = interpolator.unwrap_or(Interpolator {
                    colors: Vec::new(),
                    method: Interpolate::Rgb { gamma: 1.0 },
                    start: 0.0,
                    end: 1.0,
                });
                if let Some(nice) = &params.nice
                    && d.len() >= 2
                {
                    let mut c = Continuous::new(transform.clone(), d.clone(), vec![0.0, 1.0]);
                    nice_continuous(&mut c, type_, nice, bins.as_deref())?;
                    d = c.domain;
                }
                match type_ {
                    Type::Diverging(_) => {
                        let at = |i: usize| d.get(i).copied().unwrap_or(f64::NAN);
                        Map::Diverging(
                            Diverging {
                                transform,
                                domain: [at(0), at(1), at(2)],
                                clamp: params.clamp,
                            },
                            interpolator,
                        )
                    }
                    _ => {
                        let at = |i: usize| d.get(i).copied().unwrap_or(f64::NAN);
                        Map::Sequential(
                            Sequential {
                                transform,
                                domain: [at(0), at(d.len().max(1) - 1)],
                                clamp: params.clamp,
                            },
                            interpolator,
                        )
                    }
                }
            }
            Type::Quantile => Map::Quantile(
                Quantile::new(&numbers(&domain), range.as_ref().map_or(0, Vec::len)),
                range.unwrap_or_default(),
            ),
            Type::Quantize => {
                let d = numbers(&domain);
                let mut ends = [
                    d.first().copied().unwrap_or(0.0),
                    d.last().copied().unwrap_or(1.0),
                ];
                if let Some(nice) = &params.nice {
                    let mut c = Continuous::new(Transform::Linear, ends.to_vec(), vec![0.0, 1.0]);
                    nice_continuous(&mut c, type_, nice, bins.as_deref())?;
                    ends = [c.domain[0], c.domain[c.domain.len() - 1]];
                }
                let range = range.unwrap_or_default();
                Map::Quantize(Quantize::new(ends, range.len()), range)
            }
            Type::Threshold => {
                let range = range.unwrap_or_default();
                Map::Threshold(
                    Threshold {
                        domain: numbers(&domain),
                        count: range.len(),
                    },
                    range,
                )
            }
            Type::BinOrdinal => {
                let range = range.unwrap_or_default();
                Map::BinOrdinal(
                    BinOrdinal {
                        bins: numbers(&domain),
                        count: range.len(),
                    },
                    range,
                )
            }
            Type::Ordinal => {
                let mut unique: Vec<Value> = Vec::new();
                for d in domain {
                    if !unique.iter().any(|u| same_key(u, &d)) {
                        unique.push(d);
                    }
                }
                Map::Ordinal {
                    domain: unique,
                    range: range.unwrap_or_default(),
                    implicit: params.domain_implicit,
                }
            }
            Type::Band | Type::Point => {
                let mut unique: Vec<Value> = Vec::new();
                for d in domain {
                    if !unique.iter().any(|u| same_key(u, &d)) {
                        unique.push(d);
                    }
                }
                let n = unique.len();
                let mut band = match type_ {
                    Type::Point => Band::point(n, [0.0, 1.0], 0.0),
                    _ => Band::new(n, [0.0, 1.0]),
                };
                if let Some(padding) = params.padding {
                    band.padding_outer = padding;
                    if type_ == Type::Band {
                        band.padding_inner = padding;
                    }
                }
                if let Some(inner) = params.padding_inner
                    && type_ == Type::Band
                {
                    band.padding_inner = inner;
                }
                if let Some(outer) = params.padding_outer {
                    band.padding_outer = outer;
                }
                if let Some(align) = params.align {
                    band.align = align;
                }
                band.round = params.round;
                if let Some(range) = &range {
                    let r = numbers(range);
                    band.range = [
                        r.first().copied().unwrap_or(0.0),
                        r.last().copied().unwrap_or(1.0),
                    ];
                }
                Map::Band {
                    domain: unique,
                    band,
                }
            }
        };
        Ok(Self { type_, map, bins })
    }

    /// The value's place in the range: `undefined` where the scale has none
    /// for it.
    pub fn apply(&self, value: &Value) -> Value {
        let pick = |range: &[Value], index: Option<usize>| {
            index
                .and_then(|i| range.get(i).cloned())
                .unwrap_or_default()
        };
        let number = |v: &Value| -> Option<f64> {
            match v {
                Value::Undefined | Value::Null => None,
                v => Some(v.to_number()),
            }
        };
        match &self.map {
            Map::Identity(_) => value.clone(),
            Map::Continuous { scale, colors } => {
                let x = match number(value) {
                    Some(x) => x,
                    None => return Value::Undefined,
                };
                match colors {
                    None => Value::Number(scale.at(x)),
                    Some((colors, method)) => {
                        let t = scale.at(x);
                        if t.is_nan() {
                            return Value::Undefined;
                        }
                        let n = colors.len().saturating_sub(1);
                        let i = (t.floor().max(0.0) as usize).min(n.saturating_sub(1));
                        let rgb = match n {
                            0 => colors.first().copied().unwrap_or(nan_color()),
                            _ => method.color(colors[i], colors[i + 1], t - i as f64),
                        };
                        Value::from(rgb.to_string())
                    }
                }
            }
            Map::Sequential(scale, interpolator) => match number(value) {
                Some(x) if !x.is_nan() => Value::from(interpolator.at(scale.at(x)).to_string()),
                _ => Value::Undefined,
            },
            Map::Diverging(scale, interpolator) => match number(value) {
                Some(x) if !x.is_nan() => Value::from(interpolator.at(scale.at(x)).to_string()),
                _ => Value::Undefined,
            },
            Map::Quantile(scale, range) => pick(range, number(value).and_then(|x| scale.index(x))),
            Map::Quantize(scale, range) => pick(range, number(value).and_then(|x| scale.index(x))),
            Map::Threshold(scale, range) => pick(range, number(value).and_then(|x| scale.index(x))),
            Map::BinOrdinal(scale, range) => {
                pick(range, number(value).and_then(|x| scale.index(x)))
            }
            Map::Ordinal {
                domain,
                range,
                implicit,
            } => match domain.iter().position(|d| same_key(d, value)) {
                Some(i) if !range.is_empty() => range[i % range.len()].clone(),
                None if *implicit && !range.is_empty() => range[domain.len() % range.len()].clone(),
                _ => Value::Undefined,
            },
            Map::Band { domain, band } => match domain.iter().position(|d| same_key(d, value)) {
                Some(index) => Value::Number(band.at(index)),
                None => Value::Undefined,
            },
        }
    }

    /// A band's width; 0 for a scale without bands.
    pub fn bandwidth(&self) -> f64 {
        match &self.map {
            Map::Band { band, .. } => band.bandwidth(),
            _ => 0.0,
        }
    }

    pub fn domain(&self) -> Vec<Value> {
        let numbers = |values: &[f64]| values.iter().copied().map(Value::Number).collect();
        match &self.map {
            Map::Identity(domain) => domain.clone(),
            Map::Continuous { scale, .. } => numbers(&scale.domain),
            Map::Sequential(scale, _) => numbers(&scale.domain),
            Map::Diverging(scale, _) => numbers(&scale.domain),
            Map::Quantile(scale, _) => numbers(&scale.domain),
            Map::Quantize(scale, _) => numbers(&scale.domain),
            Map::Threshold(scale, _) => numbers(&scale.domain),
            Map::BinOrdinal(scale, _) => numbers(&scale.bins),
            Map::Ordinal { domain, .. } | Map::Band { domain, .. } => domain.clone(),
        }
    }

    /// The range, as the scale's `range()` gives it.
    pub fn range(&self) -> Vec<Value> {
        let numbers = |values: &[f64]| values.iter().copied().map(Value::Number).collect();
        match &self.map {
            Map::Identity(domain) => domain.clone(),
            Map::Continuous { scale, colors } => match colors {
                Some((colors, _)) => colors.iter().map(|c| Value::from(c.to_string())).collect(),
                None => numbers(&scale.range),
            },
            Map::Sequential(_, i) => vec![
                Value::from(i.at(0.0).to_string()),
                Value::from(i.at(1.0).to_string()),
            ],
            Map::Diverging(_, i) => vec![
                Value::from(i.at(0.0).to_string()),
                Value::from(i.at(0.5).to_string()),
                Value::from(i.at(1.0).to_string()),
            ],
            Map::Quantile(_, r)
            | Map::Quantize(_, r)
            | Map::Threshold(_, r)
            | Map::BinOrdinal(_, r)
            | Map::Ordinal { range: r, .. } => r.clone(),
            Map::Band { band, .. } => numbers(&band.range),
        }
    }

    /// The numbers of the range, for a scale that places positions.
    pub fn range_numbers(&self) -> Vec<f64> {
        self.range().iter().map(Value::to_number).collect()
    }

    /// vega-scale's `tickValues`: the scale's ticks, or its domain where it
    /// has none.
    pub fn ticks(&self, count: f64) -> Vec<Value> {
        match &self.map {
            Map::Continuous { scale, .. } => {
                scale.ticks(count).into_iter().map(Value::Number).collect()
            }
            _ => self.domain(),
        }
    }

    /// vega-scale's `tickCount` of a number: at least the bins, and with
    /// `min_step`, no more than the domain has room for.
    pub fn tick_count(&self, count: f64, min_step: Option<f64>) -> f64 {
        let mut count = match &self.bins {
            Some(bins) => count.max(bins.len() as f64),
            None => count,
        };
        let Some(min_step) = min_step else {
            return count;
        };
        let domain = self.domain();
        let (Some(first), Some(last)) = (domain.first(), domain.last()) else {
            return count;
        };
        let (a, b) = (first.to_number(), last.to_number());
        let (lo, hi) = (a.min(b), a.max(b));
        let steps = ((hi - lo) / min_step).floor();
        count = count.min(
            if steps == 0.0 || steps.is_nan() {
                1.0
            } else {
                steps
            } + 1.0,
        );
        if self.bins.is_none()
            && !self.type_.is_logarithmic()
            && !self.type_.is_temporal()
            && lo < hi
        {
            while count > 1.0 && d3::array::tick_step(lo, hi, count) < min_step {
                count -= 1.0;
            }
        }
        count
    }

    /// vega-scale's `tickFormat`: how a tick's label is written.
    pub fn tick_format(&self, count: f64, specifier: Option<&str>) -> Result<Format, String> {
        let specifier = specifier
            .map(|s| s.parse::<Specifier>().map_err(|e| e.to_string()))
            .transpose()?;
        Ok(match &self.map {
            Map::Continuous { scale, .. } => {
                let (d0, d1) = (scale.domain[0], scale.domain[scale.domain.len() - 1]);
                Format::Number(NumberFormat::span(d0, d1, count, specifier))
            }
            _ => match specifier {
                Some(specifier) => Format::Number(specifier.into()),
                None => Format::String,
            },
        })
    }
}

/// d3-scale's `InternMap` key: dates and numbers by value, anything else as
/// it is.
fn same_key(a: &Value, b: &Value) -> bool {
    a.strict_eq(b)
        || matches!((a, b), (Value::Number(x), Value::Number(y)) if x.is_nan() && y.is_nan())
}

fn nan_color() -> Rgb {
    Rgb {
        r: f64::NAN,
        g: f64::NAN,
        b: f64::NAN,
        opacity: f64::NAN,
    }
}

/// `scaleKey`: a continuous scale onto colours is sequential with a domain
/// of two, diverging with three.
fn key(type_: Type, params: &Params) -> Type {
    let colors = type_.is_continuous()
        && !type_.is_temporal()
        && !type_.is_interpolating()
        && (params.scheme.is_some()
            || params
                .range
                .as_ref()
                .is_some_and(|r| !r.is_empty() && r.iter().all(|v| matches!(v, Value::String(_)))));
    if !colors {
        return type_;
    }
    let n = match (&params.domain_raw, &params.domain) {
        (Some(raw), _) => raw.len(),
        (None, Some(d)) => d.len() + usize::from(params.domain_mid.is_some()),
        (None, None) => 0,
    };
    let kind = match type_ {
        Type::Log => Kind::Log,
        Type::Pow => Kind::Pow,
        Type::Sqrt => Kind::Sqrt,
        Type::Symlog => Kind::Symlog,
        _ => Kind::Linear,
    };
    match n {
        2 => Type::Sequential(kind),
        3 => Type::Diverging(kind),
        _ => type_,
    }
}

fn transform(type_: Type, params: &Params) -> Transform {
    match type_ {
        Type::Log => transform_kind(Kind::Log, params),
        Type::Pow => transform_kind(Kind::Pow, params),
        Type::Sqrt => transform_kind(Kind::Sqrt, params),
        Type::Symlog => transform_kind(Kind::Symlog, params),
        Type::Time => Transform::Time(LocalZone::default()),
        Type::Utc => Transform::Time(LocalZone::utc()),
        _ => Transform::Linear,
    }
}

fn transform_kind(kind: Kind, params: &Params) -> Transform {
    match kind {
        Kind::Linear => Transform::Linear,
        Kind::Log => Transform::Log {
            base: params.base.unwrap_or(10.0),
        },
        Kind::Pow => Transform::Pow {
            exponent: params.exponent.unwrap_or(1.0),
        },
        Kind::Sqrt => Transform::Pow { exponent: 0.5 },
        Kind::Symlog => Transform::Symlog {
            constant: params.constant.unwrap_or(1.0),
        },
    }
}

/// `padDomain`: the domain widened so that `padding` pixels of the range
/// lie beyond each end.
fn pad(type_: Type, domain: &[f64], range: &[f64], params: &Params) -> Vec<f64> {
    let span = (range.last().copied().unwrap_or(0.0) - range.first().copied().unwrap_or(0.0)).abs();
    let padding = params.padding.unwrap_or(0.0);
    let frac = span / (span - 2.0 * padding);
    let ends = [domain[0], domain[domain.len() - 1]];
    let d = match type_ {
        Type::Log => {
            let sign = number_sign(ends[0]);
            zoom(
                ends,
                frac,
                |x| d3::math::log(sign * x),
                |x| sign * d3::math::exp(x),
            )
        }
        Type::Sqrt => zoom(ends, frac, |x| power(x, 0.5), |x| power(x, 2.0)),
        Type::Pow => {
            let e = params.exponent.unwrap_or(1.0);
            zoom(ends, frac, |x| power(x, e), |x| power(x, 1.0 / e))
        }
        Type::Symlog => {
            let c = params.constant.unwrap_or(1.0);
            zoom(
                ends,
                frac,
                |x| number_sign(x) * (x / c).abs().ln_1p(),
                |x| number_sign(x) * x.abs().exp_m1() * c,
            )
        }
        _ => zoom(ends, frac, |x| x, |x| x),
    };
    let mut out = domain.to_vec();
    out[0] = d[0];
    let last = out.len() - 1;
    out[last] = d[1];
    out
}

fn number_sign(x: f64) -> f64 {
    d3::format::number::sign(x)
}

fn power(x: f64, e: f64) -> f64 {
    match x < 0.0 {
        true => -d3::format::number::pow(-x, e),
        false => d3::format::number::pow(x, e),
    }
}

/// vega-util's `zoom` about the domain's middle.
fn zoom(
    [d0, d1]: [f64; 2],
    scale: f64,
    lift: impl Fn(f64) -> f64,
    ground: impl Fn(f64) -> f64,
) -> [f64; 2] {
    let (d0, d1) = (lift(d0), lift(d1));
    let da = (d0 + d1) / 2.0;
    [
        ground(da + (d0 - da) * scale),
        ground(da + (d1 - da) * scale),
    ]
}

/// d3-array's `range(start, stop, step)`.
fn d3_range(start: f64, stop: f64, step: f64) -> Vec<f64> {
    let n = ((stop - start) / step).ceil().max(0.0);
    if n.is_nan() {
        return Vec::new();
    }
    (0..n as usize).map(|i| start + i as f64 * step).collect()
}

/// The nice domain a definition asks of a continuous scale.
fn nice(
    scale: &mut Continuous,
    type_: Type,
    params: &Params,
    bins: Option<&[f64]>,
) -> Result<(), Error> {
    match &params.nice {
        Some(nice) if scale.domain.len() >= 2 => nice_continuous(scale, type_, nice, bins),
        _ => Ok(()),
    }
}

/// `scale.nice(tickCount(scale, nice) || null)`.
fn nice_continuous(
    scale: &mut Continuous,
    type_: Type,
    nice: &NiceParam,
    bins: Option<&[f64]>,
) -> Result<(), Error> {
    match nice {
        NiceParam::Default => scale.nice(10.0),
        NiceParam::Count(count) => {
            let count = match bins {
                Some(bins) => count.max(bins.len() as f64),
                None => *count,
            };
            scale.nice(if count == 0.0 || count.is_nan() {
                10.0
            } else {
                count
            })
        }
        NiceParam::Interval(unit, step) => {
            if !type_.is_temporal() {
                return Err(Error::Spec(
                    "Only time and utc scales accept interval strings.".into(),
                ));
            }
            let zone = match &scale.transform {
                Transform::Time(zone) => zone.clone(),
                _ => LocalZone::utc(),
            };
            let unit = crate::time::Unit::parse(unit)?;
            let interval: Interval = unit.interval(&zone);
            let interval = match *step > 1.0 {
                true => Interval::every(
                    interval.unit,
                    *step,
                    if unit == crate::time::Unit::Isoweek {
                        1.0
                    } else {
                        0.0
                    },
                    &zone,
                ),
                false => interval,
            };
            scale.nice_to(&interval);
        }
    }
    Ok(())
}

enum SchemeRange {
    Interpolator(Interpolator),
    /// A scheme's own colours as hex, or colours sampled from a ramp.
    Colors(Vec<Value>),
}

/// `configureScheme`.
fn scheme_range(
    type_: Type,
    params: &Params,
    scheme: &SchemeParam,
    count: usize,
    method: &dyn Fn() -> Result<Interpolate, Error>,
) -> Result<SchemeRange, Error> {
    let extent = params.scheme_extent;
    let (colors, method, continuous) = match scheme {
        SchemeParam::Colors(colors) => (
            colors
                .iter()
                .map(|v| v.to_string().parse::<Rgb>().unwrap_or(nan_color()))
                .collect::<Vec<_>>(),
            method()?,
            true,
        ),
        SchemeParam::Named(name) => {
            let scheme = Scheme::from_str(name)
                .map_err(|_| Error::Spec(format!("Unrecognized scheme name: {name}")))?;
            (
                scheme.colors(),
                Interpolate::Rgb { gamma: 1.0 },
                scheme.is_continuous(),
            )
        }
    };
    let count = count as f64;
    let schemed = |n: f64| {
        params
            .scheme_count
            .filter(|c| *c != 0.0 && !c.is_nan())
            .unwrap_or(n)
    };
    let count = match type_ {
        Type::Threshold => count + 1.0,
        Type::BinOrdinal => count - 1.0,
        Type::Ordinal => schemed(if count != 0.0 { count } else { DEFAULT_COUNT }),
        Type::Quantile | Type::Quantize => schemed(DEFAULT_COUNT),
        _ => count,
    };
    let interpolator = |reverse: bool| {
        let [start, end] = extent.unwrap_or([0.0, 1.0]);
        let (start, end) = if reverse { (end, start) } else { (start, end) };
        Interpolator {
            colors: colors.clone(),
            method,
            start,
            end,
        }
    };
    if type_.is_interpolating() {
        if !continuous {
            return Err(Error::Spec(format!(
                "Scale type {} does not support interpolating color schemes.",
                type_.name()
            )));
        }
        return Ok(SchemeRange::Interpolator(interpolator(params.reverse)));
    }
    if continuous {
        // `quantizeInterpolator`: samples strictly inside the ramp.
        let ramp = interpolator(false);
        let n = count.max(0.0) as usize;
        return Ok(SchemeRange::Colors(
            (1..=n)
                .map(|i| Value::from(ramp.at(i as f64 / (n + 1) as f64).to_string()))
                .collect(),
        ));
    }
    let hex: Vec<Value> = colors
        .iter()
        .map(|c| {
            Value::from(format!(
                "#{:02x}{:02x}{:02x}",
                c.r as u8, c.g as u8, c.b as u8
            ))
        })
        .collect();
    Ok(SchemeRange::Colors(match type_ {
        Type::Ordinal => hex,
        _ => hex.into_iter().take(count.max(0.0) as usize).collect(),
    }))
}

/// A tick label formatter.
pub enum Format {
    /// `String(value)`.
    String,
    Number(NumberFormat),
}

impl Format {
    pub fn format(&self, value: &Value) -> String {
        match self {
            Self::String => value.to_string(),
            Self::Number(format) => format.format(value.to_number()),
        }
    }
}
