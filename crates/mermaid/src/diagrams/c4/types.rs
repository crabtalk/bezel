//! Pele's `diagrams/c4/types.ts`.

use indexmap::IndexMap;
use serde::Serialize;

use crate::js;

/// The header keyword of a diagram.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum C4Type {
    #[serde(rename = "C4Context")]
    Context,
    #[serde(rename = "C4Container")]
    Container,
    #[serde(rename = "C4Component")]
    Component,
    #[serde(rename = "C4Dynamic")]
    Dynamic,
    #[serde(rename = "C4Deployment")]
    Deployment,
}

/// A `$key="value"` argument.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct C4Named {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum C4Attr {
    Str(String),
    Named(C4Named),
}

/// A shape or a boundary. `kind` is set on shapes, `node_type` on deployment
/// nodes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct C4Element {
    pub alias: String,
    pub label: String,
    /// A named argument written where the alias or the label belongs. It is
    /// applied to the field it names.
    pub alias_attr: Option<C4Named>,
    pub label_attr: Option<C4Named>,
    /// The index of the boundary it is in, in the model's `boundaries`.
    pub parent: Option<usize>,
    pub ty: Option<String>,
    pub descr: Option<String>,
    pub techn: Option<String>,
    pub sprite: Option<String>,
    pub tags: Option<String>,
    pub link: Option<String>,
    pub bg_color: Option<String>,
    pub font_color: Option<String>,
    pub border_color: Option<String>,
    pub shadowing: Option<String>,
    pub shape: Option<String>,
    pub legend_text: Option<String>,
    pub legend_sprite: Option<String>,
    /// Named arguments that set no known field.
    pub extra: Option<IndexMap<String, String>>,
    /// Mermaid's typeC4Shape: person, external_system_db, container_queue
    /// and so on.
    pub kind: Option<String>,
    /// Set on deployment nodes: node, nodeL or nodeR.
    pub node_type: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct C4Rel {
    #[serde(rename = "type")]
    pub ty: String,
    pub from: String,
    pub to: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_attr: Option<C4Named>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_attr: Option<C4Named>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_attr: Option<C4Named>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub techn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sprite: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_color: Option<String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "js::json_number_opt"
    )]
    pub offset_x: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "js::json_number_opt"
    )]
    pub offset_y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<IndexMap<String, String>>,
}

/// A shape or a boundary of a model, by its index in `shapes` or
/// `boundaries`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum C4Ref {
    Shape(usize),
    Boundary(usize),
}
