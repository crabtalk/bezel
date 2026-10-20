//! What a flowchart is made of. Pele's `diagrams/flowchart/types.ts`.

use std::fmt::Debug;

use indexmap::IndexMap;
use serde::Serialize;

use crate::{
    config::{Map, Value},
    detect::Kind,
    js::JsValue,
};

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
}

/// What one diagram of the flowchart family adds to the records a
/// [`FlowStore`](super::store::FlowStore) keeps. Each addition is serialized
/// flattened into its record.
pub trait Payload {
    const KIND: Kind;
    /// Ids that never become nodes.
    const RESERVED: &'static [&'static str] = &[];
    type Node: Clone + Debug + Default + PartialEq + Serialize;
    /// Serialized after the edge's `stroke`.
    type Line: Clone + Debug + Default + PartialEq + Serialize;
    /// Serialized last in the edge.
    type Edge: Clone + Debug + Default + PartialEq + Serialize;
    /// Serialized before the subgraph's `metadata`.
    type Subgraph: Clone + Debug + Default + PartialEq + Serialize;

    /// What the diagram reads from a link's line.
    fn line(_info: &LinkInfo) -> Self::Line {
        Self::Line::default()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Node<P: Payload> {
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
    #[serde(flatten)]
    pub ext: P::Node,
}

impl<P: Payload> Node<P> {
    pub fn new(id: String) -> Self {
        Self {
            id,
            text: None,
            label_type: LabelType::Text,
            kind: None,
            styles: Vec::new(),
            classes: Vec::new(),
            dir: None,
            props: None,
            link: None,
            link_target: None,
            ext: P::Node::default(),
        }
    }

    /// Takes the `label` and `labelType` of a metadata block.
    pub fn apply_label(&mut self, fields: &Map) {
        if let Some(label) = fields.get("label").filter(|v| v.truthy()) {
            self.text = Some(label.to_js_string());
            self.label_type = match fields.get("labelType").and_then(Value::as_str) {
                Some("text") => LabelType::Text,
                Some("string") => LabelType::String,
                _ => LabelType::Markdown,
            };
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge<P: Payload> {
    pub start: String,
    pub end: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub text: String,
    pub label_type: LabelType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    #[serde(flatten)]
    pub line: P::Line,
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
    #[serde(flatten)]
    pub ext: P::Edge,
}

impl<P: Payload> Edge<P> {
    /// Takes the `animate`, `animation` and `curve` of a metadata block.
    pub fn apply_animation(&mut self, fields: &Map) {
        if let Some(value) = fields.get("animate") {
            self.animate = Some(value.clone());
        }
        if let Some(value) = fields.get("animation") {
            self.animation = Some(value.clone());
        }
        if let Some(value) = fields.get("curve") {
            self.interpolate = Some(value.clone());
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassDef {
    pub id: String,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Subgraph<P: Payload> {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: String,
    pub classes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    pub label_type: LabelType,
    #[serde(flatten)]
    pub ext: P::Subgraph,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map>,
}

impl Payload for super::Flowchart {
    const KIND: Kind = Kind::Flowchart;
    type Node = NodeAssets;
    type Line = ();
    type Edge = ();
    type Subgraph = ();
}

pub type FlowNode = Node<super::Flowchart>;

/// The icon or image a flowchart node's metadata gives it.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeAssets {
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
