//! Pele's `diagrams/venn/types.ts`.

use indexmap::IndexMap;
use serde::Serialize;

/// A set (one id) or an overlap of sets (two or more ids, sorted), with its
/// relative area.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VennSubset {
    pub sets: Vec<String>,
    #[serde(serialize_with = "crate::js::json_number")]
    pub size: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// A line of text shown inside the region of the given sets.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VennText {
    pub sets: Vec<String>,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Targets are the sorted ids of a set or overlap, or the id of a text.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VennStyle {
    pub targets: Vec<String>,
    pub styles: IndexMap<String, String>,
}
