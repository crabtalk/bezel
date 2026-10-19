//! The shapes an agentflow draws. Pele's `diagrams/agentflow/shapes.ts`.

use super::graph::GraphNode;
use crate::diagrams::flowchart::shapes::is_valid_shape as is_flowchart_shape;

/// Author-facing names for the shapes that carry meaning in an agentflow.
pub const SHAPE_ALIASES: [(&str, &str); 7] = [
    ("task", "roundedRect"),
    ("tool", "subroutine"),
    ("input", "lean-right"),
    ("decision", "diamond"),
    ("refdoc", "lin-doc"),
    ("action", "hexagon"),
    ("round", "rect"),
];

pub const REMOVED_SHAPES: [&str; 27] = [
    "doc",
    "stadium",
    "terminal",
    "circle",
    "trapezoid",
    "inv_trapezoid",
    "inv-trapezoid",
    "doublecircle",
    "double-circle",
    "typeDeclaration",
    "procs",
    "lean_left",
    "lean-left",
    "in-out",
    "cylinder",
    "ellipse",
    "odd",
    "tag-rect",
    "tagged-rectangle",
    "delay",
    "half-rounded-rectangle",
    "lin-rect",
    "lined-rectangle",
    "win-pane",
    "window-pane",
    "curv-trap",
    "curved-trapezoid",
];

pub const ALLOWED_SHAPES: [&str; 13] = [
    "roundedRect",
    "subroutine",
    "subprocess",
    "subproc",
    "framed-rectangle",
    "lean-right",
    "diamond",
    "lin-doc",
    "lined-document",
    "hexagon",
    "hex",
    "connector",
    "collapsedGroup",
];

pub const DEFAULT_SHAPE: &str = "roundedRect";

pub fn resolve_shape_alias(shape: Option<&str>) -> Option<&str> {
    let shape = shape?;
    if shape.is_empty() {
        return Some(shape);
    }
    Some(
        SHAPE_ALIASES
            .iter()
            .find(|(alias, _)| *alias == shape)
            .map_or(shape, |&(_, name)| name),
    )
}

/// Mermaid looks a shape name up with `in` on a plain object, so
/// `constructor` passes there too.
pub fn is_valid_shape(name: &str) -> bool {
    is_flowchart_shape(name) || name == "constructor"
}

/// Where the removed and unsupported shapes are reported.
pub trait ShapeSink {
    fn shape_removed(&mut self, message: String, node_id: &str);
    fn shape_unsupported(&mut self, message: String, node_id: &str);
}

/// Reduces every node's shape to one agentflow draws, reporting the removed
/// and unsupported ones.
pub fn normalise_node_shapes(nodes: &mut [GraphNode], sink: &mut dyn ShapeSink) {
    for node in nodes {
        if node.is_group {
            continue;
        }
        if node.shape.is_empty() || node.shape == "squareRect" || node.shape == "rect" {
            node.shape = DEFAULT_SHAPE.to_string();
        }
        if REMOVED_SHAPES.contains(&node.shape.as_str()) {
            sink.shape_removed(
                format!(
                    "shape \"{}\" was removed in v0.8.1, using \"{DEFAULT_SHAPE}\"",
                    node.shape
                ),
                &node.id,
            );
            node.shape = DEFAULT_SHAPE.to_string();
        } else if !ALLOWED_SHAPES.contains(&node.shape.as_str()) {
            sink.shape_unsupported(
                format!(
                    "shape \"{}\" is not supported, using \"{DEFAULT_SHAPE}\"",
                    node.shape
                ),
                &node.id,
            );
            node.shape = DEFAULT_SHAPE.to_string();
        }
    }
}
