use crate::spec::{Axis, Data, Encode, Scale, Signal, Style};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

/// A mark: items of one `type`, one for each tuple of its `from` data, or
/// one where it has none. A group mark holds marks, scales and axes of its
/// own.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Mark {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<From>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encode: Option<Encode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interactive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zindex: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aria: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlap: Option<Overlap>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<Data>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<Signal>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scales: Vec<Scale>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub axes: Vec<Axis>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<Mark>,
}

/// Where a mark's items come from.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct From {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

/// Overlapping items hidden, as the axis parser asks for its labels.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Overlap {
    pub method: Json,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separation: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound: Option<Json>,
}
