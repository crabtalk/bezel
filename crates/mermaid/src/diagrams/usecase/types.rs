//! What a use case diagram is made of. Pele's `diagrams/usecase/types.ts`.
//! Offsets count UTF-16 units of the source.

use indexmap::IndexMap;
use serde::Serialize;
use serde_repr::Serialize_repr;

use crate::config::Value;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelType {
    #[default]
    Text,
    Markdown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ActorType {
    Normal,
    Hollow,
    Awesome,
    Icon,
}

impl ActorType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Hollow => "hollow",
            Self::Awesome => "awesome",
            Self::Icon => "icon",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UseCaseShape {
    Ellipse,
    Rect,
}

impl UseCaseShape {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ellipse => "ellipse",
            Self::Rect => "rect",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BoundaryType {
    Rect,
    Package,
}

impl BoundaryType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Rect => "rect",
            Self::Package => "package",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RelationshipType {
    Association,
    Include,
    Extend,
    Generalization,
}

impl RelationshipType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Association => "association",
            Self::Include => "include",
            Self::Extend => "extend",
            Self::Generalization => "generalization",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Animation {
    Fast,
    Slow,
}

impl Animation {
    pub fn name(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Slow => "slow",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Direction {
    TB,
    TD,
    BT,
    RL,
    LR,
}

impl Direction {
    /// The way the drawing's ranks run.
    pub fn flow(self) -> crate::geom::Direction {
        match self {
            Self::TB | Self::TD => crate::geom::Direction::TopToBottom,
            Self::BT => crate::geom::Direction::BottomToTop,
            Self::RL => crate::geom::Direction::RightToLeft,
            Self::LR => crate::geom::Direction::LeftToRight,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Actor,
    Usecase,
    Boundary,
    Json,
    Edge,
}

impl SymbolKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Actor => "actor",
            Self::Usecase => "usecase",
            Self::Boundary => "boundary",
            Self::Json => "json",
            Self::Edge => "edge",
        }
    }
}

/// Start and end offsets in the source, end exclusive.
pub type Span = [usize; 2];

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Actor {
    pub id: String,
    pub label: String,
    pub label_type: LabelType,
    #[serde(rename = "type")]
    pub kind: ActorType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub business: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stereotype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UseCase {
    pub id: String,
    pub label: String,
    pub label_type: LabelType,
    pub shape: UseCaseShape,
    pub business: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stereotype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemBoundary {
    pub id: String,
    pub label: String,
    pub label_type: LabelType,
    #[serde(rename = "type")]
    pub kind: BoundaryType,
    pub members: Vec<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

/// An association's arrow, serialized as Mermaid's number for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize_repr)]
#[repr(u8)]
pub enum Arrow {
    Solid = 0,
    Back = 1,
    Line = 2,
    Circle = 3,
    Cross = 4,
    CircleReversed = 5,
    CrossReversed = 6,
}

impl Arrow {
    /// Ends in a circle or a cross.
    pub fn is_decorated(self) -> bool {
        self as u8 > Arrow::Line as u8
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Relationship {
    pub id: String,
    pub explicit_id: bool,
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub kind: RelationshipType,
    pub arrow_type: Arrow,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_type: Option<LabelType>,
    pub minlen: usize,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub animate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<Animation>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsecaseNote {
    pub id: String,
    pub target: String,
    pub label: String,
    pub label_type: LabelType,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsecaseJsonNode {
    pub id: String,
    /// An object.
    pub value: Value,
    /// The keys of each object in the order they were written, by JSON
    /// Pointer.
    pub property_order: IndexMap<String, Vec<String>>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClassDef {
    pub id: String,
    pub styles: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataOccurrence {
    pub key: String,
    pub span: Span,
    pub key_span: Span,
    pub value_span: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeOccurrence {
    pub id: String,
    pub span: Span,
    pub id_span: Span,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_span: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defines: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stereotype_span: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Vec<MetadataOccurrence>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_spans: Option<Vec<Span>>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeOccurrence {
    pub id: String,
    pub span: Span,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_span: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_span: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Vec<MetadataOccurrence>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StatementKind {
    Node,
    Edge,
    Group,
    Note,
    Json,
    Metadata,
    EdgeMetadata,
    ClassDef,
    ClassAssign,
    Style,
    Direction,
    AccTitle,
    AccDescr,
    Comment,
    Blank,
}

/// One statement of the source with the positions of its parts, as in
/// Mermaid's use case AST.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphStatement {
    pub kind: StatementKind,
    pub span: Span,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodes: Option<Vec<NodeOccurrence>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edges: Option<Vec<EdgeOccurrence>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_span: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_span: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_span: Option<Span>,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_span: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<GraphStatement>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Vec<MetadataOccurrence>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_spans: Option<Vec<Span>>,
}

impl GraphStatement {
    pub fn new(kind: StatementKind) -> Self {
        Self {
            kind,
            span: [0, 0],
            nodes: None,
            edges: None,
            group: None,
            id_span: None,
            title_span: None,
            end_span: None,
            reference: None,
            ref_span: None,
            children: None,
            metadata: None,
            class_spans: None,
        }
    }
    pub fn with_nodes(kind: StatementKind, nodes: Vec<NodeOccurrence>) -> Self {
        Self {
            nodes: Some(nodes),
            ..Self::new(kind)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsecaseModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub actors: IndexMap<String, Actor>,
    pub use_cases: IndexMap<String, UseCase>,
    pub system_boundaries: IndexMap<String, SystemBoundary>,
    pub relationships: Vec<Relationship>,
    pub notes: IndexMap<String, UsecaseNote>,
    pub json_nodes: IndexMap<String, UsecaseJsonNode>,
    pub class_defs: IndexMap<String, ClassDef>,
    pub direction: Direction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub acc_title: String,
    pub acc_descr: String,
    /// The accessible description as written, before the indentation of its
    /// lines is dropped.
    pub acc_description: String,
    pub source: String,
    pub header_span: Span,
    pub statements: Vec<GraphStatement>,
}

impl UsecaseModel {
    pub fn empty() -> Self {
        Self {
            kind: "usecase",
            actors: IndexMap::new(),
            use_cases: IndexMap::new(),
            system_boundaries: IndexMap::new(),
            relationships: Vec::new(),
            notes: IndexMap::new(),
            json_nodes: IndexMap::new(),
            class_defs: IndexMap::new(),
            direction: Direction::LR,
            title: None,
            acc_title: String::new(),
            acc_descr: String::new(),
            acc_description: String::new(),
            source: String::new(),
            header_span: [0, 0],
            statements: Vec::new(),
        }
    }
}
