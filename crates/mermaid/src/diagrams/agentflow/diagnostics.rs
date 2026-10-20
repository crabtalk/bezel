//! Mermaid's diagnostic vocabulary for agentflow. Only the ids in
//! [`EMITTED`] are raised by a parser; the rest are reserved for tools that
//! check a diagram's meaning. Pele's `diagrams/agentflow/diagnostics.ts`.

use super::types::DiagnosticId;

pub const ALL: [DiagnosticId; 12] = [
    DiagnosticId::ShapeUnsupported,
    DiagnosticId::ShapeRemoved,
    DiagnosticId::EdgeOperatorUnsupported,
    DiagnosticId::ReferenceEdgeLabelRejected,
    DiagnosticId::ConnectorRefUnresolved,
    DiagnosticId::ConnectorRefNotAConnector,
    DiagnosticId::MetadataKeyMisapplied,
    DiagnosticId::DuplicateIdNode,
    DiagnosticId::ReservedSyntheticId,
    DiagnosticId::ContainmentViolation,
    DiagnosticId::EdgeSemanticContradiction,
    DiagnosticId::FlowNoInput,
];

pub const EMITTED: [DiagnosticId; 3] = [
    DiagnosticId::ShapeUnsupported,
    DiagnosticId::ShapeRemoved,
    DiagnosticId::ContainmentViolation,
];

impl DiagnosticId {
    /// Whether a parser never raises this.
    pub fn is_reserved(self) -> bool {
        !EMITTED.contains(&self)
    }
}
