//! Mermaid source as a canvas, parsed and laid out by `mermaid-rs-renderer`.
//!
//! Flowchart, state and class diagrams; every other kind answers `None`.
//! Labels are measured at `font_size` against the system fonts, which the
//! first call in a process loads.

use std::panic::{self, AssertUnwindSafe};

use mermaid_rs_renderer as mmdr;

use crate::model::{Canvas, Edge, End, GROUP, Node, Shape, Stroke, TEXT};

/// The canvas `source` describes, its labels measured at `font_size`, or
/// `None` for source the renderer cannot read or a kind it does not lay out
/// as a graph.
pub fn import(source: &str, font_size: f32) -> Option<Canvas> {
    let theme = mmdr::Theme {
        font_size,
        ..mmdr::Theme::modern()
    };
    let config = mmdr::LayoutConfig::default();
    // The parser sees every keystroke of a fence being typed. A panic in it is
    // a diagram that does not draw, not an app that goes down.
    let layout = panic::catch_unwind(AssertUnwindSafe(|| {
        let parsed = mmdr::parse_mermaid_strict(source).ok()?;
        Some(mmdr::compute_layout(&parsed.graph, &theme, &config))
    }))
    .ok()??;
    if !matches!(
        layout.kind,
        mmdr::DiagramKind::Flowchart | mmdr::DiagramKind::State | mmdr::DiagramKind::Class
    ) {
        return None;
    }

    let round = |value: f32| value.round() as i64;
    let mut canvas = Canvas::default();
    for (ix, group) in layout.subgraphs.iter().enumerate() {
        canvas.nodes.push(Node {
            id: format!("subgraph-{ix}"),
            kind: GROUP.into(),
            x: round(group.x),
            y: round(group.y),
            width: round(group.width),
            height: round(group.height),
            label: (!group.label.is_empty()).then(|| group.label.clone()),
            ..Node::default()
        });
    }
    for node in layout.nodes.values().filter(|node| !node.hidden) {
        let text = node.label.lines.join("\n");
        canvas.nodes.push(Node {
            id: node.id.clone(),
            kind: TEXT.into(),
            x: round(node.x),
            y: round(node.y),
            width: round(node.width),
            height: round(node.height),
            text: (!text.trim().is_empty()).then_some(text),
            shape: Some(shape(node.shape)),
            ..Node::default()
        });
    }
    for (ix, edge) in layout.edges.iter().enumerate() {
        let label = edge
            .label
            .as_ref()
            .map(|label| label.lines.join("\n"))
            .filter(|label| !label.trim().is_empty());
        canvas.edges.push(Edge {
            from_end: Some(end(edge.arrow_start, edge.start_decoration)),
            to_end: Some(end(edge.arrow_end, edge.end_decoration)),
            label,
            points: edge
                .points
                .iter()
                .map(|&(x, y)| [round(x), round(y)])
                .collect(),
            style: match edge.style {
                mmdr::EdgeStyle::Solid => None,
                mmdr::EdgeStyle::Dotted => Some(Stroke::Dotted),
                mmdr::EdgeStyle::Thick => Some(Stroke::Thick),
            },
            ..Edge::new(format!("edge-{ix}"), &edge.from, &edge.to)
        });
    }
    Some(canvas)
}

fn shape(shape: mmdr::NodeShape) -> Shape {
    use mmdr::NodeShape::*;
    match shape {
        RoundRect => Shape::Round,
        Stadium => Shape::Stadium,
        Circle | DoubleCircle => Shape::Circle,
        Diamond => Shape::Diamond,
        Hexagon => Shape::Hexagon,
        Parallelogram | ParallelogramAlt | Trapezoid | TrapezoidAlt | Asymmetric => {
            Shape::Parallelogram
        }
        Cylinder => Shape::Cylinder,
        Rectangle | ForkJoin | Subroutine | ActorBox | MindmapDefault | Text => Shape::Rect,
    }
}

fn end(arrow: bool, decoration: Option<mmdr::EdgeDecoration>) -> End {
    match decoration {
        Some(mmdr::EdgeDecoration::Circle) => End::Circle,
        Some(mmdr::EdgeDecoration::Cross) => End::Cross,
        _ if arrow => End::Arrow,
        _ => End::None,
    }
}
