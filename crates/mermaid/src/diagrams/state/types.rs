//! Pele's `diagrams/state/types.ts`.

use indexmap::IndexMap;
use serde::{Serialize, Serializer};

use super::graph::{StateGraph, StateShape};
use crate::{
    diagrams::common::{db::Titles, style_class::StyleClass},
    geom::Direction,
};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StateNote {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    pub text: String,
}

/// A description: one text, or a title and the lines under it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Descr {
    One(String),
    Many(Vec<String>),
}

impl Descr {
    /// An empty text. A list, even an empty one, is not.
    pub fn is_empty(&self) -> bool {
        matches!(self, Descr::One(text) if text.is_empty())
    }

    /// The texts, one or many.
    pub fn texts(&self) -> Vec<String> {
        match self {
            Descr::One(text) => vec![text.clone()],
            Descr::Many(list) => list.clone(),
        }
    }
}

/// The `stmt` tag of a state, which every state carries wherever it sits.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StateTag;

impl Serialize for StateTag {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str("state")
    }
}

/// What a state statement declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StateKind {
    Default,
    Fork,
    Join,
    Choice,
    /// A region of a composite, between dividers.
    Divider,
}

impl StateKind {
    /// The shape a state of this kind is drawn as.
    pub fn shape(self) -> StateShape {
        match self {
            StateKind::Default => StateShape::Rect,
            StateKind::Fork => StateShape::Fork,
            StateKind::Join => StateShape::Join,
            StateKind::Choice => StateShape::Choice,
            StateKind::Divider => StateShape::Divider,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateStmt {
    pub stmt: StateTag,
    pub id: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<StateKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Descr>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descriptions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<Vec<Stmt>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<StateNote>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub styles: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_styles: Option<Vec<String>>,
}

impl StateStmt {
    pub fn new(id: impl Into<String>, kind: Option<StateKind>) -> Self {
        Self {
            id: id.into(),
            kind,
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RelationStmt {
    pub state1: StateStmt,
    pub state2: StateStmt,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClassDefStmt {
    pub id: String,
    pub classes: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleStmt {
    pub id: String,
    pub style_class: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DirectionStmt {
    pub value: Direction,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClickStmt {
    pub id: StateStmt,
    pub url: String,
    pub tooltip: String,
}

/// Mermaid's grammar also leaves the text of statements that have no effect
/// in a document (`hide empty description`, `scale`, a bare `state name`, a
/// floating note); those are the texts.
#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Text(String),
    State(Box<StateStmt>),
    Relation(Box<RelationStmt>),
    ClassDef(ClassDefStmt),
    Style(StyleStmt),
    ApplyClass(StyleStmt),
    Dir(DirectionStmt),
    Click(Box<ClickStmt>),
}

/// A statement's fields after its `stmt` tag.
#[derive(Serialize)]
struct Tagged<'a, T> {
    stmt: &'static str,
    #[serde(flatten)]
    inner: &'a T,
}

impl Serialize for Stmt {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Stmt::Text(text) => s.serialize_str(text),
            Stmt::State(inner) => inner.serialize(s),
            Stmt::Relation(inner) => Tagged {
                stmt: "relation",
                inner,
            }
            .serialize(s),
            Stmt::ClassDef(inner) => Tagged {
                stmt: "classDef",
                inner,
            }
            .serialize(s),
            Stmt::Style(inner) => Tagged {
                stmt: "style",
                inner,
            }
            .serialize(s),
            Stmt::ApplyClass(inner) => Tagged {
                stmt: "applyClass",
                inner,
            }
            .serialize(s),
            Stmt::Dir(inner) => Tagged { stmt: "dir", inner }.serialize(s),
            Stmt::Click(inner) => Tagged {
                stmt: "click",
                inner,
            }
            .serialize(s),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateRelation {
    pub id1: String,
    pub id2: String,
    pub relation_title: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StateLink {
    pub url: String,
    pub tooltip: String,
}

/// Pele's `StateDb` as `JSON.stringify` writes it.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub root_doc: Vec<Stmt>,
    pub states: IndexMap<String, StateStmt>,
    pub relations: Vec<StateRelation>,
    pub classes: IndexMap<String, StyleClass>,
    pub links: IndexMap<String, StateLink>,
    pub graph: StateGraph,
    #[serde(flatten)]
    pub titles: Titles,
    pub divider_count: usize,
}
