//! What a flowchart is made of. Pele's `diagrams/flowchart/types.ts`.

use indexmap::IndexMap;
use serde::Serialize;

use crate::config::{Map, Value};

/// A parsed flowchart: the database the parser fills.
pub type FlowchartModel = super::db::FlowDb;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelType {
    Text,
    String,
    Markdown,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlowText {
    pub text: String,
    pub kind: LabelType,
}

/// What a statement says about a vertex; `None` and empty fields say
/// nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Vertex<'a> {
    pub label: Option<&'a FlowText>,
    pub shape: Option<&'a str>,
    pub styles: &'a [String],
    /// One `field: value` pair for the vertex's props.
    pub prop: Option<(&'a str, &'a str)>,
    /// The text of its `@{ … }` blocks.
    pub metadata: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowNode {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub label_type: LabelType,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub styles: Vec<String>,
    pub classes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub props: Option<IndexMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub img: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "crate::js::json_number_opt"
    )]
    pub asset_width: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "crate::js::json_number_opt"
    )]
    pub asset_height: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowEdge {
    pub start: String,
    pub end: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub text: String,
    pub label_type: LabelType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<Vec<String>>,
    pub classes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub is_user_defined_id: bool,
    /// A curve name, or whatever value the edge's metadata gave `curve`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interpolate: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animate: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowClassDef {
    pub id: String,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowSubgraph {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: String,
    pub classes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    pub label_type: LabelType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinkInfo {
    pub kind: String,
    pub stroke: String,
    pub length: Option<i32>,
    pub text: Option<FlowText>,
    pub id: Option<String>,
}

/// What a statement adds to the subgraph around it.
#[derive(Clone, Debug, PartialEq)]
pub enum DocItem {
    Str(String),
    List(Vec<String>),
    Dir(String),
}
