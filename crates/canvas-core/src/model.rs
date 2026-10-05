//! The document: [JSON Canvas 1.0](https://jsoncanvas.org/spec/1.0/), field
//! for field, plus a `version`, a node's [`Shape`], an edge's route
//! [`Edge::points`] and [`Stroke`], and the [`End`]s past `none` and `arrow`.
//! [`crate::json_canvas`] translates to and from the spec.
//!
//! A field neither names lands in `extra` and is written back, so a canvas
//! another app wrote survives a save here. A node's `type` is a string rather
//! than an enum for the same reason — an app's own kind is a renderer away, not
//! a fork of the format.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

pub const TEXT: &str = "text";
pub const FILE: &str = "file";
pub const LINK: &str = "link";
pub const GROUP: &str = "group";

/// The version [`Canvas::to_json`] stamps: the release of bezel that wrote it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Canvas {
    /// The bezel release that wrote it. `None` for one translated from JSON
    /// Canvas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Paint order: the first is at the bottom.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<Node>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<Edge>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    /// `text`, `file`, `link`, `group`, or an app's own.
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(deserialize_with = "int")]
    pub x: i64,
    #[serde(deserialize_with = "int")]
    pub y: i64,
    #[serde(deserialize_with = "int")]
    pub width: i64,
    #[serde(deserialize_with = "int")]
    pub height: i64,
    /// A preset `"1"`–`"6"` or a hex string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Starts with `#`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subpath: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_style: Option<String>,
    /// `None` reads as [`Shape::Rect`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<Shape>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub id: String,
    /// `curve`, or an app's own. The spec names no edge type; ours is written
    /// back as any field we add is, and another reader ignores it.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub from_node: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_side: Option<Side>,
    /// `None` reads as [`End::None`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_end: Option<End>,
    pub to_node: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_side: Option<Side>,
    /// `None` reads as [`End::Arrow`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_end: Option<End>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Where it runs, end to end, in canvas units. Empty leaves the route to
    /// its kind.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "points"
    )]
    pub points: Vec<[i64; 2]>,
    /// `None` reads as [`Stroke::Solid`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Stroke>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The outline a node paints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    #[default]
    Rect,
    /// Corners rounded past the card's.
    Round,
    /// Ends fully round.
    Stadium,
    /// The ellipse the box holds.
    Circle,
    Diamond,
    Hexagon,
    /// Leaning right.
    Parallelogram,
    Cylinder,
}

/// How an edge's line is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stroke {
    #[default]
    Solid,
    Dashed,
    Dotted,
    Thick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum End {
    None,
    Arrow,
    Circle,
    Cross,
}

impl Canvas {
    pub fn parse(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }

    /// Stamped with [`VERSION`].
    pub fn to_json(&self) -> String {
        let stamped = Self {
            version: Some(VERSION.to_owned()),
            ..self.clone()
        };
        serde_json::to_string_pretty(&stamped).expect("a canvas is always valid JSON")
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|node| node.id == id)
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|node| node.id == id)
    }

    pub fn edge(&self, id: &str) -> Option<&Edge> {
        self.edges.iter().find(|edge| edge.id == id)
    }

    /// Every node by id, for looking many up at once.
    pub fn lookup(&self) -> HashMap<&str, &Node> {
        self.nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect()
    }

    /// `n` ids no node or edge holds, distinct from each other.
    pub fn mint_n(&self, n: usize) -> Vec<String> {
        let taken = |id: &str| {
            self.nodes.iter().any(|node| node.id == id)
                || self.edges.iter().any(|edge| edge.id == id)
        };
        (self.nodes.len() + self.edges.len()..)
            .map(|n| format!("{n:016x}"))
            .filter(|id| !taken(id))
            .take(n)
            .collect()
    }

    /// An id no node or edge holds, in the 16 hex digits other writers use.
    pub fn mint(&self) -> String {
        self.mint_n(1)
            .pop()
            .expect("an unbounded range has a free id")
    }
}

impl Edge {
    pub fn new(id: impl Into<String>, from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            from_node: from.into(),
            to_node: to.into(),
            ..Self::default()
        }
    }
}

/// The spec says integers; some writers emit fractions anyway.
fn int<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    f64::deserialize(deserializer).map(|value| value.round() as i64)
}

fn points<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<[i64; 2]>, D::Error> {
    Vec::<[f64; 2]>::deserialize(deserializer).map(|points| {
        points
            .into_iter()
            .map(|[x, y]| [x.round() as i64, y.round() as i64])
            .collect()
    })
}
