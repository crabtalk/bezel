//! Pele's `diagrams/requirement/types.ts`.

use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub requirement_id: String,
    pub text: String,
    pub risk: String,
    pub verify_method: String,
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
    pub kind: String,
    pub src: String,
    pub dst: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementClass {
    pub id: String,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}
