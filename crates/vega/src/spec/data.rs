use crate::spec::{Many, Transform};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

/// A data set: inline `values`, a `url`, or derived from one or more
/// `source`s, run through its `transform`s.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Data {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Many<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<Format>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transform: Vec<Transform>,
}

/// How a data set's text or values become tuples: vega-loader's `read`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Format {
    /// `json`, `csv`, `tsv`, `dsv` or `topojson`.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse: Option<Parse>,
    /// A json format's path to the array of values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,
    /// A dsv format's delimiter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<String>,
    /// A topojson format's object to extract as features.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    /// A topojson format's object to extract as a mesh.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh: Option<String>,
}

/// `"auto"`, or a type for each field, by its name.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Parse {
    Auto(Auto),
    Fields(IndexMap<String, String>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Auto {
    Auto,
}
