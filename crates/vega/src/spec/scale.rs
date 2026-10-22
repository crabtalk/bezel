use crate::spec::SignalRef;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

/// A scale: a `domain` of data values mapped onto a `range`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Scale {
    pub name: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<Domain>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_inner: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_outer: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nice: Option<Nice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zero: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reverse: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clamp: Option<Json>,
}

/// A scale's domain.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Domain {
    Signal(SignalRef),
    Values(Vec<Json>),
    Data(DataDomain),
}

/// The values of `field` in `data`, or of each of `fields`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DataDomain {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<Sort>,
}

/// How a discrete domain's values are ordered.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Sort {
    Flag(bool),
    By(SortBy),
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SortBy {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
}

/// A scale's range.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Range {
    Signal(SignalRef),
    Named(String),
    Step(StepRange),
    Values(Vec<Json>),
}

/// A band or point scale's range: `step` pixels a band.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StepRange {
    pub step: Json,
}

/// Whether and how a scale's domain extends to round values.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Nice {
    Flag(bool),
    Count(f64),
    Signal(SignalRef),
}
