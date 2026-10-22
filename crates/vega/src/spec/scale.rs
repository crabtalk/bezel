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
    pub domain_min: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_max: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_mid: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_raw: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_implicit: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interpolate: Option<Interpolate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bins: Option<Bins>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exponent: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constant: Option<Json>,
}

/// A scale's domain.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Domain {
    Signal(SignalRef),
    Values(Vec<Json>),
    Data(DataDomain),
}

/// The values of `field` in `data`, or of each of `fields`: a field of
/// `data`, a reference to a field of another data set, or literal values.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DataDomain {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<DomainField>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<Sort>,
}

/// One of a multi-field domain's fields.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DomainField {
    Name(String),
    Signal(SignalRef),
    Values(Vec<Json>),
    Ref { data: String, field: String },
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
    Values(Vec<Json>),
    /// `width`, `height`, or a range the config names.
    Named(String),
    Step(StepRange),
    Scheme(SchemeRange),
    /// A discrete scale's range read from data, as a domain is.
    Data(DataDomain),
}

/// A band or point scale's range: `step` pixels a band.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StepRange {
    pub step: Json,
}

/// A colour scheme, by name or as its colours.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SchemeRange {
    pub scheme: Json,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extent: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<Json>,
}

/// Whether and how a scale's domain extends to round values.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Nice {
    Flag(bool),
    Count(f64),
    Signal(SignalRef),
    Interval(NiceInterval),
    /// A time unit's name.
    Unit(String),
}

/// A time scale's nice domain: whole `step`s of an `interval`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NiceInterval {
    pub interval: Json,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<Json>,
}

/// How a scale interpolates its range: a method's name, or the method and
/// its gamma.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Interpolate {
    Signal(SignalRef),
    Name(String),
    Method {
        #[serde(rename = "type")]
        type_: Json,
        #[serde(skip_serializing_if = "Option::is_none")]
        gamma: Option<Json>,
    },
}

/// A scale's bin boundaries: the values, or a `step` from `start` to
/// `stop`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Bins {
    Signal(SignalRef),
    Values(Vec<Json>),
    Step {
        step: Json,
        #[serde(skip_serializing_if = "Option::is_none")]
        start: Option<Json>,
        #[serde(skip_serializing_if = "Option::is_none")]
        stop: Option<Json>,
    },
}
