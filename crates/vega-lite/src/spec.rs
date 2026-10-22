//! A Vega-Lite spec as serde reads it. A property outside these types fails
//! to parse.

use serde::{Deserialize, Deserializer};
use serde_json::Value as Json;

/// A top-level unit spec: one mark over one data source.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TopLevel {
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    pub description: Option<String>,
    pub data: Option<Data>,
    pub mark: Mark,
    #[serde(default)]
    pub encoding: Encoding,
    pub width: Option<Json>,
    pub height: Option<Json>,
}

/// Inline `values`, or a `url` the Vega view loads.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Data {
    pub values: Option<Json>,
    pub url: Option<String>,
}

/// A mark type, or a definition of one.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Mark {
    Type(String),
    Def(MarkDef),
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkDef {
    #[serde(rename = "type")]
    pub type_: String,
}

/// The channels a mark encodes.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Encoding {
    pub x: Option<FieldDef>,
    pub y: Option<FieldDef>,
}

/// A field definition: a `field` of a data `type`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldDef {
    pub field: String,
    #[serde(rename = "type")]
    pub type_: Type,
    pub title: Option<Json>,
    /// `Some(None)` where `axis` is `null`.
    #[serde(default, deserialize_with = "present")]
    pub axis: Option<Option<Axis>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Type {
    Quantitative,
    Ordinal,
    Nominal,
    Temporal,
}

impl Type {
    /// `isDiscrete`.
    pub fn is_discrete(self) -> bool {
        matches!(self, Self::Nominal | Self::Ordinal)
    }
}

/// An axis definition.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Axis {
    pub label_angle: Option<f64>,
    pub title: Option<Json>,
    pub grid: Option<bool>,
}

fn present<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}
