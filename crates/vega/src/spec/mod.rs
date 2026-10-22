//! A [Vega spec](https://vega.github.io/vega/docs/specification/) as serde
//! reads and writes it. A property outside these types fails to parse.

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use serde_json::Value as Json;

mod axis;
mod data;
mod encode;
mod mark;
mod scale;
mod transform;

pub use axis::{Axis, AxisEncode};
pub use data::{Auto, Data, Format, Parse};
pub use encode::{
    Block, Channel, Encode, Entry, FieldObject, FieldRef, Operand, Rule, ScaleRef, string_value,
};
pub use mark::{From, Mark, Overlap};
pub use scale::{DataDomain, Domain, Nice, Range, Scale, Sort, SortBy, StepRange};
pub use transform::{
    Aggregate, Bin, Collect, Compare, Countpattern, Cross, Density, Distribution, Dotbin, Extent,
    Field, Filter, Flatten, Fold, Formula, Identifier, Impute, Joinaggregate, Kde, Loess, Lookup,
    Offset, Order, Param, Pivot, Project, Quantile, Regression, Sample, Sequence, Stack, Timeunit,
    Transform, Window,
};

/// A top-level Vega spec.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Spec {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autosize: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encode: Option<Encode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<Data>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<Signal>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<Mark>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scales: Vec<Scale>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub axes: Vec<Axis>,
}

/// A style name, or several in increasing precedence.
pub type Style = Many<String>;

/// A signal: a named value, set from `value` and recomputed from `update`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Signal {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update: Option<String>,
}

/// `{"signal": expression}`, read only from an object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SignalRef {
    pub signal: String,
}

impl<'de> Deserialize<'de> for SignalRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut object = serde_json::Map::deserialize(deserializer)?;
        match (object.remove("signal"), object.is_empty()) {
            (Some(Json::String(signal)), true) => Ok(Self { signal }),
            _ => Err(D::Error::custom("expected {\"signal\": expression}")),
        }
    }
}

/// An `Option` field's value, `Some` even where it is written `null`.
pub(crate) fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

/// One item, or a list of them, as written.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Many<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> Many<T> {
    pub fn items(&self) -> &[T] {
        match self {
            Self::One(item) => std::slice::from_ref(item),
            Self::Many(items) => items,
        }
    }
}
