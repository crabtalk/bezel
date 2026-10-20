//! Pele's `diagrams/c4/types.ts`.

use std::fmt;

use indexmap::IndexMap;
use serde::{Serialize, Serializer};

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
    /// Set on shapes.
    pub kind: Option<ShapeKind>,
    /// Set on deployment nodes.
    pub node_type: Option<NodeType>,
}

/// What a shape is, Mermaid's typeC4Shape: `person`, `external_system_db`,
/// `container_queue` and so on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapeKind {
    pub base: ShapeBase,
    pub external: bool,
    pub store: Option<Store>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeBase {
    Person,
    System,
    Container,
    Component,
}

/// A shape that holds data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Store {
    Db,
    Queue,
}

impl ShapeKind {
    pub const fn new(base: ShapeBase, external: bool, store: Option<Store>) -> Self {
        Self {
            base,
            external,
            store,
        }
    }
}

/// Mermaid's name, its words joined by `_`.
impl fmt::Display for ShapeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.external {
            f.write_str("external_")?;
        }
        f.write_str(match self.base {
            ShapeBase::Person => "person",
            ShapeBase::System => "system",
            ShapeBase::Container => "container",
            ShapeBase::Component => "component",
        })?;
        f.write_str(match self.store {
            None => "",
            Some(Store::Db) => "_db",
            Some(Store::Queue) => "_queue",
        })
    }
}

impl Serialize for ShapeKind {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

/// The keyword of a deployment node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum NodeType {
    #[serde(rename = "node")]
    Node,
    #[serde(rename = "nodeL")]
    NodeL,
    #[serde(rename = "nodeR")]
    NodeR,
}

/// The keyword of a relation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelKind {
    #[default]
    Rel,
    Birel,
    RelU,
    RelD,
    RelL,
    RelR,
    RelB,
}

impl RelKind {
    /// Whether the relation has an arrowhead at its start.
    pub fn arrow_at_start(self) -> bool {
        matches!(self, Self::Birel | Self::RelB)
    }

    /// Whether the relation has an arrowhead at its end.
    pub fn arrow_at_end(self) -> bool {
        self != Self::RelB
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct C4Rel {
    #[serde(rename = "type")]
    pub ty: RelKind,
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
