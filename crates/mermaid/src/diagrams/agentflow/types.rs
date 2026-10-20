//! What an agentflow is made of. Pele's `diagrams/agentflow/types.ts`.

use serde::Serialize;

use crate::{config::Map, detect::Kind};

pub use crate::diagrams::flowchart::types::{
    ArrowType, DocItem, FlowDir, FlowText, LabelType, LinkInfo, Node, Payload, Stroke, Vertex,
};

/// A parsed agentflow: the database the parser fills.
pub type AgentflowModel = super::db::AgentflowDb;

/// Metadata an element's `@{ … }` blocks gave it.
pub type Metadata = Map;

/// A source span as Jison reports it: lines start at 1, columns at 0.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Loc {
    pub first_line: usize,
    pub first_column: usize,
    pub last_line: usize,
    pub last_column: usize,
}

impl Loc {
    /// The empty span at `line` and `column`.
    pub fn at(line: usize, column: usize) -> Self {
        Self {
            first_line: line,
            first_column: column,
            last_line: line,
            last_column: column,
        }
    }

    /// The span from the start of this one to the end of `end`.
    pub fn to(self, end: Loc) -> Self {
        Self {
            last_line: end.last_line,
            last_column: end.last_column,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VertexKind {
    Tool,
    Action,
    Input,
    Refdoc,
    Decision,
    Connector,
    Task,
}

impl VertexKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Tool => "tool",
            Self::Action => "action",
            Self::Input => "input",
            Self::Refdoc => "refdoc",
            Self::Decision => "decision",
            Self::Connector => "connector",
            Self::Task => "task",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeSemantic {
    Sequence,
    Reference,
    Failure,
}

impl Payload for super::Agentflow {
    const KIND: Kind = Kind::Agentflow;
    /// Mermaid reserves this id and creates no node for it.
    const RESERVED: &'static [&'static str] = &["connectors"];
    type Node = NodeExt;
    type Line = LineExt;
    type Edge = EdgeExt;
    type Subgraph = SubgraphExt;

    fn line(info: &LinkInfo) -> LineExt {
        let edge_semantic = match (info.kind, info.stroke) {
            (ArrowType::Point, Stroke::Normal) => Some(EdgeSemantic::Sequence),
            (ArrowType::Cross, Stroke::Normal) => Some(EdgeSemantic::Failure),
            (ArrowType::Open, Stroke::Dotted) => Some(EdgeSemantic::Reference),
            _ => None,
        };
        LineExt { edge_semantic }
    }
}

pub type AgentNode = Node<super::Agentflow>;

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeExt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_connector: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineExt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_semantic: Option<EdgeSemantic>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct EdgeExt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SubgraphExt {
    #[serde(rename = "type")]
    pub kind: &'static str,
}

impl Default for SubgraphExt {
    fn default() -> Self {
        Self { kind: "flow" }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementPosition {
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub start_index: usize,
    pub end_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StatementType {
    Vertex,
    Edge,
    Subgraph,
    Connector,
    Attachment,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ElementMapping {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: StatementType,
    pub position: ElementPosition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiagnosticId {
    ShapeUnsupported,
    ShapeRemoved,
    EdgeOperatorUnsupported,
    ReferenceEdgeLabelRejected,
    ConnectorRefUnresolved,
    ConnectorRefNotAConnector,
    MetadataKeyMisapplied,
    DuplicateIdNode,
    ReservedSyntheticId,
    ContainmentViolation,
    EdgeSemanticContradiction,
    FlowNoInput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub id: DiagnosticId,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<ElementPosition>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticVertex {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertex_kind: Option<VertexKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticEdge {
    pub start: String,
    pub end: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<ArrowType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_semantic: Option<EdgeSemantic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSubgraph {
    pub id: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub nodes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<FlowDir>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticConnector {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

/// The diagram with presentation-only controls left out, for tools that
/// read what it means.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<FlowDir>,
    pub vertices: Vec<SemanticVertex>,
    pub edges: Vec<SemanticEdge>,
    pub sub_graphs: Vec<SemanticSubgraph>,
    pub connectors: Vec<SemanticConnector>,
    pub diagnostics: Vec<Diagnostic>,
}
