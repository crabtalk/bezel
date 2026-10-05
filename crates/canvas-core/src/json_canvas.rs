//! Translation between this format and [JSON Canvas
//! 1.0](https://jsoncanvas.org/spec/1.0/).
//!
//! Reading a JSON Canvas file names no `version`. A file whose own fields
//! reuse this format's names reads as this format's.

use crate::model::{Canvas, End};

/// A JSON Canvas document, as this format.
pub fn import(json: &str) -> serde_json::Result<Canvas> {
    let mut canvas = Canvas::parse(json)?;
    canvas.version = None;
    Ok(canvas)
}

/// `canvas` as JSON Canvas: no version, no shapes, routes or strokes, and an
/// end the spec has no value for drawn as an arrow. Fields neither format
/// names are kept.
pub fn export(canvas: &Canvas) -> String {
    let mut spec = canvas.clone();
    spec.version = None;
    for node in &mut spec.nodes {
        node.shape = None;
    }
    for edge in &mut spec.edges {
        edge.points.clear();
        edge.style = None;
        for end in [&mut edge.from_end, &mut edge.to_end] {
            if matches!(end, Some(End::Circle | End::Cross)) {
                *end = Some(End::Arrow);
            }
        }
    }
    serde_json::to_string_pretty(&spec).expect("a canvas is always valid JSON")
}
