//! What a flowchart is made of. Pele's `diagrams/flowchart/types.ts`.

use std::fmt::Debug;

use indexmap::IndexMap;
use serde::{Serialize, Serializer};

use crate::{
    config::{Map, Value},
    detect::Kind,
    geom::{Direction, Size},
    js::{self, JsValue},
    svg::edges::Marker,
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
    pub kind: Option<ArrowType>,
    pub text: String,
    pub label_type: LabelType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
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
pub struct Subgraph<P: Payload> {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: String,
    pub classes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<FlowDir>,
    pub label_type: LabelType,
    #[serde(flatten)]
    pub ext: P::Subgraph,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map>,
}

impl<P: Payload> Subgraph<P> {
    /// Whether its metadata's `view` is `collapsed`.
    pub fn is_collapsed(&self) -> bool {
        self.metadata
            .as_ref()
            .and_then(|metadata| metadata.get("view"))
            .and_then(Value::as_str)
            == Some("collapsed")
    }
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

impl NodeAssets {
    /// The image, unless blank.
    pub fn image(&self) -> Option<&str> {
        self.img.as_deref().filter(|img| !img.is_empty())
    }

    /// The icon, unless blank.
    pub fn icon(&self) -> Option<&str> {
        self.icon.as_deref().filter(|icon| !icon.is_empty())
    }

    /// The image's or icon's size: the one the metadata gives, where it is
    /// above 0 and at most 4000, else `fallback`.
    pub fn size(&self, fallback: f64) -> Size {
        let pick = |value: Option<f64>| match value {
            Some(value) if value > 0.0 && value <= 4000.0 => value,
            _ => fallback,
        };
        Size::new(pick(self.asset_width), pick(self.asset_height))
    }

    /// Whether the image or icon sits above the label.
    pub fn on_top(&self) -> bool {
        self.pos.as_deref() == Some("t")
    }
}

/// The heads a link draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArrowType {
    #[default]
    Open,
    Point,
    Cross,
    Circle,
    DoublePoint,
    DoubleCross,
    DoubleCircle,
    Invalid,
}

impl ArrowType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Open => "arrow_open",
            Self::Point => "arrow_point",
            Self::Cross => "arrow_cross",
            Self::Circle => "arrow_circle",
            Self::DoublePoint => "double_arrow_point",
            Self::DoubleCross => "double_arrow_cross",
            Self::DoubleCircle => "double_arrow_circle",
            Self::Invalid => "INVALID",
        }
    }

    /// This head at both ends. Only a single head has a double.
    pub fn doubled(self) -> Self {
        match self {
            Self::Point => Self::DoublePoint,
            Self::Cross => Self::DoubleCross,
            Self::Circle => Self::DoubleCircle,
            other => other,
        }
    }

    /// The marker it draws at an end, if any.
    pub fn marker(self) -> Option<Marker> {
        match self {
            Self::Point | Self::DoublePoint => Some(Marker::Arrow),
            Self::Cross | Self::DoubleCross => Some(Marker::Cross),
            Self::Circle | Self::DoubleCircle => Some(Marker::Circle),
            Self::Open | Self::Invalid => None,
        }
    }

    /// Whether it draws a head at both ends.
    pub fn is_double(self) -> bool {
        matches!(
            self,
            Self::DoublePoint | Self::DoubleCross | Self::DoubleCircle
        )
    }
}

impl Serialize for ArrowType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

/// The line of a link.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stroke {
    #[default]
    Normal,
    Thick,
    Dotted,
    Invisible,
    Invalid,
}

impl Stroke {
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Thick => "thick",
            Self::Dotted => "dotted",
            Self::Invisible => "invisible",
            Self::Invalid => "INVALID",
        }
    }
}

impl Serialize for Stroke {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinkInfo {
    pub kind: ArrowType,
    pub stroke: Stroke,
    pub length: Option<i32>,
    pub text: Option<FlowText>,
    pub id: Option<String>,
}

impl LinkInfo {
    /// What the halves of a link that disagree draw.
    pub fn invalid() -> Self {
        Self {
            kind: ArrowType::Invalid,
            stroke: Stroke::Invalid,
            ..Self::default()
        }
    }
}

/// What a statement adds to the subgraph around it.
#[derive(Clone, Debug, PartialEq)]
pub enum DocItem {
    Str(String),
    List(Vec<String>),
    Dir(FlowDir),
}

/// A direction by the name the grammar reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FlowDir {
    Tb,
    Td,
    Bt,
    Lr,
    Rl,
    /// Read as a direction, drawn top to bottom.
    Br,
}

impl FlowDir {
    /// The direction a `graph` header's direction token names: `<`, `^`,
    /// `>` and `v` by the arrow, and `TD` as `TB`.
    pub fn from_header(token: &str) -> Self {
        let token = js::trim(token);
        if token.contains('v') {
            Self::Tb
        } else if token.contains('>') {
            Self::Lr
        } else if token.contains('^') {
            Self::Bt
        } else if token.contains('<') {
            Self::Rl
        } else {
            match token {
                "BT" => Self::Bt,
                "LR" => Self::Lr,
                "RL" => Self::Rl,
                "BR" => Self::Br,
                _ => Self::Tb,
            }
        }
    }

    /// The direction drawn.
    pub fn direction(self) -> Direction {
        match self {
            Self::Tb | Self::Td | Self::Br => Direction::TopToBottom,
            Self::Bt => Direction::BottomToTop,
            Self::Lr => Direction::LeftToRight,
            Self::Rl => Direction::RightToLeft,
        }
    }
}
