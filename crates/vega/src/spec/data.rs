use crate::spec::Many;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

/// A data set: inline `values`, a `url`, or derived from a `source`, run
/// through its `transform`s.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Data {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<Json>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transform: Vec<Transform>,
}

/// A data transform, by its `type`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Transform {
    Stack(Stack),
    Filter(Filter),
}

/// `filter`: the tuples `expr` is truthy on.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub expr: String,
}

/// `stack`: `field` summed into `as` within each `groupby` group, in `sort`
/// order.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Stack {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groupby: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<Compare>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<Offset>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<[String; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Offset {
    Zero,
    Center,
    Normalize,
}

/// A comparator: fields compared in turn, each in its order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Compare {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<Many<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Many<Order>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Order {
    Ascending,
    Descending,
}
