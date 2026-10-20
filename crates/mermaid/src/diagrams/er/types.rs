//! Pele's `diagrams/er/types.ts`.

use serde::Serialize;

use crate::geom::Direction;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Cardinality {
    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
    OnlyOne,
    MdParent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Identification {
    NonIdentifying,
    Identifying,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ErAttribute {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub keys: Vec<String>,
    pub comment: String,
}

/// What the parser hands over for one attribute; keys and comment are
/// present only when written.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedAttribute {
    pub kind: String,
    pub name: String,
    pub keys: Option<Vec<String>>,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErEntity {
    pub id: String,
    pub label: String,
    pub alias: String,
    pub attributes: Vec<ErAttribute>,
    pub css_classes: String,
    pub css_styles: Vec<String>,
}

/// `card_b` sits at entity A's end of the line and `card_a` at entity B's,
/// as in Mermaid.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelSpec {
    pub card_a: Cardinality,
    pub rel_type: Identification,
    pub card_b: Cardinality,
}

/// `entity_a` and `entity_b` hold an entity's id, or a subgraph's id when
/// the name was a subgraph at the time.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErRelationship {
    pub entity_a: String,
    pub role_a: String,
    pub entity_b: String,
    pub rel_spec: RelSpec,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelType {
    Text,
    String,
    Markdown,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErSubgraph {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: String,
    pub classes: Vec<String>,
    pub css_styles: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<Direction>,
    pub label_type: LabelType,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DocItem {
    Name(String),
    Dir(Direction),
}
