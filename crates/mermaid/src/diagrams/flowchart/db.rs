//! What the flowchart parser builds. Pele's `diagrams/flowchart/db.ts`.

use serde::{Serialize, Serializer, ser::SerializeMap};

use super::{
    Flowchart,
    shapes::is_valid_shape,
    store::{Element, FlowStore},
    types::{LinkInfo, Vertex},
};
use crate::{
    config::Value,
    detect::Kind,
    error::Error,
    js::{self, JsValue},
    yaml::parse_yaml,
};

/// The flowchart model. Serializes as Pele's `FlowDb` does through
/// `JSON.stringify`, its bookkeeping included.
pub type FlowDb = FlowStore<Flowchart>;

fn destruct_start_link(raw: &str) -> LinkInfo {
    let mut text = js::trim(raw);
    let mut kind = "arrow_open";
    match text.as_bytes().first() {
        Some(b'<') => {
            kind = "arrow_point";
            text = &text[1..];
        }
        Some(b'x') => {
            kind = "arrow_cross";
            text = &text[1..];
        }
        Some(b'o') => {
            kind = "arrow_circle";
            text = &text[1..];
        }
        _ => {}
    }
    let mut stroke = "normal";
    if text.contains('=') {
        stroke = "thick";
    }
    if text.contains('.') {
        stroke = "dotted";
    }
    LinkInfo {
        kind: kind.into(),
        stroke: stroke.into(),
        ..LinkInfo::default()
    }
}

fn destruct_end_link(raw: &str) -> LinkInfo {
    let text = js::trim(raw);
    let mut line = text.get(..text.len().saturating_sub(1)).unwrap_or("");
    let mut kind = String::from("arrow_open");
    let last = text.as_bytes().last().copied();
    let first = if text.starts_with('x') {
        Some(b'x')
    } else if text.starts_with('<') {
        Some(b'<')
    } else if text.starts_with('o') {
        Some(b'o')
    } else {
        None
    };
    let (single, opener) = match last {
        Some(b'x') => ("arrow_cross", b'x'),
        Some(b'>') => ("arrow_point", b'<'),
        Some(b'o') => ("arrow_circle", b'o'),
        _ => ("", 0),
    };
    if !single.is_empty() {
        kind = single.into();
        if first == Some(opener) {
            kind = format!("double_{kind}");
            line = line.get(1..).unwrap_or("");
        }
    }
    let mut stroke = "normal";
    let mut length = js::len(line) as i32 - 1;
    if line.starts_with('=') {
        stroke = "thick";
    }
    if line.starts_with('~') {
        stroke = "invisible";
    }
    let dots = line.matches('.').count() as i32;
    if dots > 0 {
        stroke = "dotted";
        length = dots;
    }
    LinkInfo {
        kind,
        stroke: stroke.into(),
        length: Some(length),
        ..LinkInfo::default()
    }
}

/// The link a `LINK` token and the `START_LINK` before it, if any, draw.
pub fn destruct_link(end: &str, start: Option<&str>) -> LinkInfo {
    let info = destruct_end_link(end);
    let Some(start) = start.filter(|start| !start.is_empty()) else {
        return info;
    };
    let invalid = || LinkInfo {
        kind: "INVALID".into(),
        stroke: "INVALID".into(),
        ..LinkInfo::default()
    };
    let mut start_info = destruct_start_link(start);
    if start_info.stroke != info.stroke {
        return invalid();
    }
    if start_info.kind == "arrow_open" {
        start_info.kind = info.kind;
    } else {
        if start_info.kind != info.kind {
            return invalid();
        }
        start_info.kind = format!("double_{}", start_info.kind);
    }
    if start_info.kind == "double_arrow" {
        start_info.kind = "double_arrow_point".into();
    }
    start_info.length = info.length;
    start_info
}

impl FlowDb {
    /// Applies a `@{ … }` block to what `id` names: a subgraph's or node's
    /// metadata, or an edge's animation and curve.
    pub fn add_metadata(&mut self, id: &str, metadata: &str) -> Result<(), Error> {
        if js::trim(id).is_empty() {
            return Ok(());
        }
        let yaml = if metadata.contains('\n') {
            format!("{metadata}\n")
        } else {
            format!("{{\n{metadata}\n}}")
        };
        let parsed = parse_yaml(&yaml).map_err(|error| {
            Error::syntax(format!(
                "Metadata of \"{id}\" is not valid YAML. {}",
                error.0
            ))
            .of(Kind::Flowchart)
        })?;
        let Value::Object(doc) = parsed else {
            self.add_vertex(id, Vertex::default());
            return Ok(());
        };

        if let Some(subgraph) = self.subgraph_mut(id) {
            subgraph.metadata.get_or_insert_default().extend(doc);
            return Ok(());
        }
        let node = match self.add_vertex(id, Vertex::default()) {
            Some(Element::Node(node)) => node,
            Some(Element::Edge(edge)) => {
                edge.apply_animation(&doc);
                return Ok(());
            }
            None => return Ok(()),
        };

        if let Some(shape) = doc.get("shape").filter(|v| v.truthy()) {
            let shape = shape.to_js_string();
            if shape != shape.to_lowercase() || shape.contains('_') {
                return Err(Error::semantic(format!(
                    "No such shape: {shape}. Shape names should be lowercase."
                ))
                .of(Kind::Flowchart));
            }
            if !is_valid_shape(&shape) {
                return Err(Error::semantic(format!("No such shape: {shape}.")).of(Kind::Flowchart));
            }
            node.kind = Some(shape);
        }
        let label = doc.get("label").and_then(Value::as_str);
        let blank_label = label.is_none_or(|label| js::trim(label).is_empty());
        node.apply_label(&doc);
        let assets = &mut node.ext;
        if let Some(icon) = doc.get("icon").filter(|v| v.truthy()) {
            assets.icon = Some(icon.to_js_string());
            if blank_label && node.text.as_deref() == Some(id) {
                node.text = Some(String::new());
            }
        }
        if let Some(form) = doc.get("form").filter(|v| v.truthy()) {
            assets.form = Some(form.to_js_string());
        }
        if let Some(pos) = doc.get("pos").filter(|v| v.truthy()) {
            assets.pos = Some(pos.to_js_string());
        }
        if let Some(img) = doc.get("img").filter(|v| v.truthy()) {
            assets.img = Some(img.to_js_string());
            if blank_label && node.text.as_deref() == Some(id) {
                node.text = Some(String::new());
            }
        }
        if let Some(constraint) = doc.get("constraint").filter(|v| v.truthy()) {
            assets.constraint = Some(constraint.to_js_string());
        }
        if let Some(w) = doc.get("w").filter(|v| v.truthy()) {
            assets.asset_width = Some(w.to_number());
        }
        if let Some(h) = doc.get("h").filter(|v| v.truthy()) {
            assets.asset_height = Some(h.to_number());
        }
        Ok(())
    }
}

impl Serialize for FlowDb {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("type", "flowchart")?;
        if let Some(direction) = &self.direction {
            map.serialize_entry("direction", direction)?;
        }
        map.serialize_entry("nodes", &self.nodes)?;
        map.serialize_entry("edges", &self.edges)?;
        map.serialize_entry("subgraphs", &self.subgraphs)?;
        map.serialize_entry("classes", &self.classes)?;
        map.serialize_entry("tooltips", &self.tooltips)?;
        if let Some(style) = &self.default_edge_style {
            map.serialize_entry("defaultEdgeStyle", style)?;
        }
        if let Some(interpolate) = &self.default_interpolate {
            map.serialize_entry("defaultInterpolate", interpolate)?;
        }
        if let Some(title) = &self.title {
            map.serialize_entry("title", title)?;
        }
        if let Some(acc_title) = &self.acc_title {
            map.serialize_entry("accTitle", acc_title)?;
        }
        if let Some(acc_descr) = &self.acc_descr {
            map.serialize_entry("accDescr", acc_descr)?;
        }
        map.serialize_entry("subgraphLookup", &self.subgraphs_by_id())?;
        map.serialize_entry("members", &self.members)?;
        map.serialize_entry("edgeById", &self.edges_by_id())?;
        map.serialize_entry("pairCount", &self.pair_count)?;
        map.serialize_entry("subCount", &self.sub_count)?;
        map.serialize_entry("options", &self.options)?;
        map.end()
    }
}
