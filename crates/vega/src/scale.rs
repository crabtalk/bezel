//! A configured scale, as vega-encode's `Scale` operator sets up a
//! vega-scale scale: a domain, a range, and what a scale type does between
//! them.

use crate::value::Value;
use d3::{
    format::{NumberFormat, Specifier},
    scale::{Band, Continuous},
};

/// A scale's type, by Vega's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    Band,
    Point,
    Linear,
}

impl std::str::FromStr for Type {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, String> {
        Ok(match name {
            "band" => Self::Band,
            "point" => Self::Point,
            "linear" => Self::Linear,
            other => return Err(format!("Unrecognized scale type: \"{other}\"")),
        })
    }
}

impl Type {
    /// vega-scale's `isDiscrete`.
    pub fn is_discrete(self) -> bool {
        matches!(self, Self::Band | Self::Point)
    }
}

/// A scale ready to map values.
#[derive(Clone, Debug)]
pub enum Scale {
    /// A band or point scale over `domain`.
    Band {
        domain: Vec<Value>,
        band: Band,
    },
    Linear(Continuous),
}

impl Scale {
    /// The value's place in the range: `undefined` for a value a discrete
    /// domain lacks.
    pub fn apply(&self, value: &Value) -> Value {
        match self {
            Self::Band { domain, band } => match domain.iter().position(|d| d.strict_eq(value)) {
                Some(index) => Value::Number(band.at(index)),
                None => Value::Undefined,
            },
            Self::Linear(scale) => Value::Number(scale.at(value.to_number())),
        }
    }

    /// A band's width; 0 for a scale without bands.
    pub fn bandwidth(&self) -> f64 {
        match self {
            Self::Band { band, .. } => band.bandwidth(),
            Self::Linear(_) => 0.0,
        }
    }

    pub fn domain(&self) -> Vec<Value> {
        match self {
            Self::Band { domain, .. } => domain.clone(),
            Self::Linear(scale) => scale.domain.iter().copied().map(Value::Number).collect(),
        }
    }

    pub fn range(&self) -> Vec<f64> {
        match self {
            Self::Band { band, .. } => band.range.to_vec(),
            Self::Linear(scale) => scale.range.clone(),
        }
    }

    /// vega-scale's `tickValues`: the scale's ticks, or its domain where it
    /// has none.
    pub fn ticks(&self, count: f64) -> Vec<Value> {
        match self {
            Self::Band { domain, .. } => domain.clone(),
            Self::Linear(scale) => scale.ticks(count).into_iter().map(Value::Number).collect(),
        }
    }

    /// vega-scale's `tickFormat`: how a tick's label is written.
    pub fn tick_format(&self, count: f64, specifier: Option<&str>) -> Result<Format, String> {
        let specifier = specifier
            .map(|s| s.parse::<Specifier>().map_err(|e| e.to_string()))
            .transpose()?;
        Ok(match self {
            Self::Linear(scale) => {
                let (d0, d1) = (scale.domain[0], scale.domain[scale.domain.len() - 1]);
                Format::Number(NumberFormat::span(d0, d1, count, specifier))
            }
            Self::Band { .. } => match specifier {
                Some(specifier) => Format::Number(specifier.into()),
                None => Format::String,
            },
        })
    }
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

/// What a scale definition gives the `Scale` operator, its signals and data
/// references resolved.
#[derive(Clone, Debug, Default)]
pub struct Params {
    pub domain: Option<Vec<Value>>,
    pub range: Option<Vec<f64>>,
    pub range_step: Option<f64>,
    pub padding: Option<f64>,
    pub padding_inner: Option<f64>,
    pub padding_outer: Option<f64>,
    pub align: Option<f64>,
    pub round: bool,
    pub reverse: bool,
    pub clamp: bool,
    pub zero: Option<bool>,
    /// `true`, or a tick count.
    pub nice: Option<f64>,
}

impl Scale {
    /// vega-encode's `Scale` transform for a `type` scale.
    pub fn new(type_: Type, params: &Params) -> Self {
        match type_ {
            Type::Band | Type::Point => {
                let domain = params.domain.clone().unwrap_or_default();
                let count = domain.len();
                let mut band = match type_ {
                    Type::Point => Band::point(count, [0.0, 1.0], 0.0),
                    _ => Band::new(count, [0.0, 1.0]),
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
                let range = match params.range_step {
                    Some(step) => {
                        let outer = params.padding_outer.or(params.padding).unwrap_or(0.0);
                        let inner = match type_ {
                            Type::Point => 1.0,
                            _ => params.padding_inner.or(params.padding).unwrap_or(0.0),
                        };
                        Some(vec![0.0, step * Band::space(count, inner, outer)])
                    }
                    None => params.range.clone(),
                };
                if let Some(mut range) = range {
                    if params.reverse {
                        range.reverse();
                    }
                    band.range = [range[0], range[range.len() - 1]];
                }
                Self::Band { domain, band }
            }
            Type::Linear => {
                let mut domain: Vec<f64> = params
                    .domain
                    .iter()
                    .flatten()
                    .map(Value::to_number)
                    .collect();
                let zero = params.zero.unwrap_or(true);
                if zero && !domain.is_empty() {
                    let n = match domain.len() {
                        1 => 1,
                        len => len - 1,
                    };
                    if domain[0] > 0.0 {
                        domain[0] = 0.0;
                    }
                    if domain.get(n).is_some_and(|d| *d < 0.0) {
                        domain[n] = 0.0;
                    }
                }
                let mut scale =
                    Continuous::new(d3::scale::Transform::Linear, domain, vec![0.0, 1.0]);
                if let Some(count) = params.nice {
                    scale.nice(count);
                }
                if let Some(mut range) = params.range.clone() {
                    if params.reverse {
                        range.reverse();
                    }
                    scale.range = range;
                }
                scale.round = params.round;
                scale.clamp = params.clamp;
                Self::Linear(scale)
            }
        }
    }
}
