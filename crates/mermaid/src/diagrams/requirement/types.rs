//! Pele's `diagrams/requirement/types.ts`.

use serde::{Serialize, Serializer};

/// What a requirement statement declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequirementType {
    Requirement,
    Functional,
    Interface,
    Performance,
    Physical,
    DesignConstraint,
}

impl RequirementType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Requirement => "Requirement",
            Self::Functional => "Functional Requirement",
            Self::Interface => "Interface Requirement",
            Self::Performance => "Performance Requirement",
            Self::Physical => "Physical Requirement",
            Self::DesignConstraint => "Design Constraint",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Risk {
    Low,
    Medium,
    High,
}

impl Risk {
    pub fn name(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifyMethod {
    Analysis,
    Demonstration,
    Inspection,
    Test,
}

impl VerifyMethod {
    pub fn name(self) -> &'static str {
        match self {
            Self::Analysis => "Analysis",
            Self::Demonstration => "Demonstration",
            Self::Inspection => "Inspection",
            Self::Test => "Test",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relationship {
    Contains,
    Copies,
    Derives,
    Satisfies,
    Verifies,
    Refines,
    Traces,
}

impl Relationship {
    pub fn name(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Copies => "copies",
            Self::Derives => "derives",
            Self::Satisfies => "satisfies",
            Self::Verifies => "verifies",
            Self::Refines => "refines",
            Self::Traces => "traces",
        }
    }
}

macro_rules! serialize_by_name {
    ($($name:ident),*) => {$(
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.name())
            }
        }
    )*};
}
serialize_by_name!(RequirementType, Risk, VerifyMethod, Relationship);

/// A field the diagram may leave out, written as the empty string then.
pub(crate) fn or_empty<V: Serialize, S: Serializer>(
    value: &Option<V>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => value.serialize(s),
        None => s.serialize_str(""),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: RequirementType,
    pub requirement_id: String,
    pub text: String,
    #[serde(serialize_with = "or_empty")]
    pub risk: Option<Risk>,
    #[serde(serialize_with = "or_empty")]
    pub verify_method: Option<VerifyMethod>,
    pub css_styles: Vec<String>,
    pub classes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementElement {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub doc_ref: String,
    pub css_styles: Vec<String>,
    pub classes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Relation {
    #[serde(rename = "type")]
    pub kind: Relationship,
    pub src: String,
    pub dst: String,
}
