//! Pele's `diagrams/class/types.ts`.

use indexmap::IndexSet;
use serde::{Serialize, Serializer};

use super::members::ClassMember;

/// A relation's end marker: Mermaid's relationType number, or none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationEnd {
    Type(u8),
    None,
}

impl Serialize for RelationEnd {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Type(n) => s.serialize_u8(*n),
            Self::None => s.serialize_str("none"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassNode {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub label: String,
    pub css_classes: String,
    pub methods: Vec<ClassMember>,
    pub members: Vec<ClassMember>,
    pub annotations: Vec<String>,
    pub styles: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub have_callback: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    pub type1: RelationEnd,
    pub type2: RelationEnd,
    pub line_type: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassRelation {
    pub id1: String,
    pub id2: String,
    pub relation_title1: String,
    pub relation_title2: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub relation: Relation,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassNote {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    pub text: String,
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassInterface {
    pub id: String,
    pub label: String,
    pub class_id: String,
}

/// A namespace. Its classes, notes and children are held by the model and
/// named here by id; the model serializes them in place.
#[derive(Clone, Debug, PartialEq)]
pub struct NamespaceNode {
    pub id: String,
    pub label: String,
    pub classes: IndexSet<String>,
    pub notes: IndexSet<String>,
    pub children: IndexSet<String>,
    pub parent: Option<String>,
    /// False for the ancestors created on the way to a dotted name such as
    /// `A.B.C`.
    pub explicit: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleClass {
    pub id: String,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}
