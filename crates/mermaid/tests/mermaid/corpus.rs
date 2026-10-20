//! Every corpus entry draws: Mermaid's documentation examples, by kind.

use std::{fs, path::Path};

use mermaid::{Metrics, Options, diagrams::Model, parse, render};
use serde_json::{Map, Value, json};

fn draws(file: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/mermaid/corpus/{file}"));
    let sources: Vec<String> =
        serde_json::from_str(&fs::read_to_string(&path).expect("corpus file"))
            .expect("corpus JSON");
    for (i, source) in sources.iter().enumerate() {
        let drawing = render(source, &Options::default(), &Metrics)
            .unwrap_or_else(|error| panic!("{file} #{i}: {error}"));
        assert!(drawing.svg.starts_with("<svg"), "{file} #{i}");
        assert!(drawing.width > 0.0 && drawing.height > 0.0, "{file} #{i}");
    }
}

#[test]
fn info() {
    draws("info-docs.json");
}

#[test]
fn pie() {
    draws("pie-docs.json");
}

#[test]
fn radar() {
    draws("radar-docs.json");
}

#[test]
fn treemap() {
    draws("treemap-docs.json");
}

#[test]
fn sankey() {
    draws("sankey-docs.json");
}

#[test]
fn quadrant() {
    draws("quadrant-docs.json");
}

#[test]
fn xychart() {
    draws("xychart-docs.json");
}

#[test]
fn packet() {
    draws("packet-docs.json");
}

#[test]
fn treeview() {
    draws("treeview-docs.json");
}

#[test]
fn git() {
    draws("git-docs.json");
}

#[test]
fn mindmap() {
    draws("mindmap-docs.json");
}

#[test]
fn railroad() {
    draws("railroad-docs.json");
}

#[test]
fn venn() {
    draws("venn-docs.json");
}

#[test]
fn ishikawa() {
    draws("ishikawa-docs.json");
}

#[test]
fn journey() {
    draws("journey-docs.json");
}

#[test]
fn kanban() {
    draws("kanban-docs.json");
}

#[test]
fn gantt() {
    draws("gantt-docs.json");
}

#[test]
fn timeline() {
    draws("timeline-docs.json");
}

#[test]
fn class() {
    draws("class-docs.json");
}

#[test]
fn flowchart() {
    draws("flowchart-docs.json");
}

#[test]
fn state() {
    draws("state-docs.json");
}

#[test]
fn er() {
    draws("er-docs.json");
}

#[test]
fn requirement() {
    draws("requirement-docs.json");
}

#[test]
fn sequence() {
    draws("sequence-docs.json");
}

#[test]
fn cynefin() {
    draws("cynefin-docs.json");
}

#[test]
fn wardley() {
    draws("wardley-docs.json");
}

#[test]
fn eventmodeling() {
    draws("eventmodeling-docs.json");
}

#[test]
fn block() {
    draws("block-docs.json");
}

#[test]
fn architecture() {
    draws("architecture-docs.json");
}

#[test]
fn c4() {
    draws("c4-docs.json");
}

#[test]
fn swimlane() {
    draws("swimlane-docs.json");
}

#[test]
fn agentflow() {
    draws("agentflow-docs.json");
}

#[test]
fn usecase() {
    draws("usecase-docs.json");
}

/// Mermaid's palette length and default curve where the reference was made.
const REFERENCE_PALETTE: usize = 12;
const REFERENCE_CURVE: &str = "basis";

fn to<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("JSON")
}

/// The fields of each object named, where they are set.
fn pick(value: Value, keys: &[&str]) -> Value {
    let Value::Object(object) = value else {
        return value;
    };
    Value::Object(
        keys.iter()
            .filter_map(|&key| object.get(key).map(|v| (key.to_string(), v.clone())))
            .collect(),
    )
}

/// What `tests/support/agentflow-reference.ts` in Pele projects from a
/// model, for comparison with what Mermaid made of the same source.
fn reference_projection(source: &str) -> Result<Value, String> {
    let parsed = parse(source, &Options::default()).map_err(|error| error.message)?;
    let Model::Agentflow(db) = parsed.model else {
        return Err("not an agentflow".into());
    };
    let mut db = (*db).clone();
    let graph = db.build_graph(REFERENCE_PALETTE, Some(REFERENCE_CURVE));
    let model = serde_json::to_value(&db).expect("model JSON");
    let nodes: Vec<Value> = graph
        .nodes
        .iter()
        .map(|n| {
            let classes = format!(
                "{}{}",
                n.css_classes,
                n.kind
                    .map_or(String::new(), |kind| format!(" af-kind-{}", kind.name()))
            );
            let mut node = Map::new();
            node.insert("id".into(), json!(n.id));
            if let Some(label) = &n.label {
                node.insert("label".into(), json!(label));
            }
            node.insert("labelType".into(), to(&n.label_type));
            node.insert("shape".into(), json!(n.shape));
            node.insert("isGroup".into(), json!(n.is_group));
            if let Some(parent) = &n.parent_id {
                node.insert("parentId".into(), json!(parent));
            }
            node.insert("cssCompiledStyles".into(), json!(n.css_compiled_styles));
            node.insert(
                "cssClasses".into(),
                json!(classes.split_whitespace().collect::<Vec<_>>().join(" ")),
            );
            if let Some(dir) = n.dir {
                node.insert("dir".into(), json!(dir));
            }
            for (key, value) in [
                ("link", &n.link),
                ("linkTarget", &n.link_target),
                ("tooltip", &n.tooltip),
            ] {
                if let Some(value) = value {
                    node.insert(key.into(), json!(value));
                }
            }
            if let Some(slot) = n.color_index {
                node.insert("colorIndex".into(), json!(slot));
            }
            if let Some(metadata) = &n.metadata {
                node.insert("metadata".into(), to(metadata));
            }
            Value::Object(node)
        })
        .collect();
    let layout_edges: Vec<Value> = graph
        .edges
        .iter()
        .map(|e| {
            let mut edge = Map::new();
            edge.insert("id".into(), json!(e.id));
            edge.insert("start".into(), json!(e.start));
            edge.insert("end".into(), json!(e.end));
            edge.insert(
                "type".into(),
                json!(e.kind.map_or("normal", |kind| kind.name())),
            );
            edge.insert("label".into(), json!(e.label));
            edge.insert("labelType".into(), to(&e.label_type));
            if let Some(thickness) = &e.thickness {
                edge.insert("thickness".into(), json!(thickness));
            }
            if let Some(minlen) = e.minlen {
                edge.insert("minlen".into(), json!(minlen));
            }
            edge.insert("arrowTypeStart".into(), json!("none"));
            edge.insert(
                "arrowTypeEnd".into(),
                json!(e.arrow_type_end.map_or("none", |marker| marker.name())),
            );
            edge.insert("cssCompiledStyles".into(), json!(e.css_compiled_styles));
            edge.insert("style".into(), json!(e.style));
            if let Some(curve) = &e.curve {
                edge.insert("curve".into(), json!(curve));
            }
            if let Some(metadata) = &e.metadata {
                edge.insert("metadata".into(), to(metadata));
            }
            edge.insert("isUserDefinedId".into(), json!(e.is_user_defined_id));
            Value::Object(edge)
        })
        .collect();
    let mappings: Vec<Value> = db
        .mappings
        .iter()
        .map(|m| {
            let p = &m.position;
            json!([
                m.id,
                to(&m.kind),
                p.start_line,
                p.start_column,
                p.end_line,
                p.end_column,
                p.start_index,
                p.end_index
            ])
        })
        .collect();
    let vertices: Vec<Value> = model["nodes"]
        .as_object()
        .map(|nodes| {
            nodes
                .values()
                .map(|v| {
                    pick(
                        v.clone(),
                        &[
                            "id",
                            "text",
                            "labelType",
                            "type",
                            "styles",
                            "classes",
                            "link",
                            "linkTarget",
                            "metadata",
                            "isConnector",
                            "props",
                        ],
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let mut out = Map::new();
    if let Some(direction) = &db.store.direction {
        out.insert("direction".into(), json!(direction));
    }
    out.insert(
        "accTitle".into(),
        json!(db.store.titles.acc_title.clone().unwrap_or_default()),
    );
    out.insert(
        "accDescr".into(),
        json!(db.store.titles.acc_descr.clone().unwrap_or_default()),
    );
    out.insert("vertices".into(), json!(vertices));
    out.insert("edges".into(), model["edges"].clone());
    if let Some(style) = &db.store.default_edge_style {
        out.insert("defaultStyle".into(), json!(style));
    }
    if let Some(interpolate) = &db.store.default_interpolate {
        out.insert("defaultInterpolate".into(), json!(interpolate));
    }
    out.insert("subGraphs".into(), model["subgraphs"].clone());
    out.insert(
        "classes".into(),
        json!(
            model["classes"]
                .as_object()
                .map(|c| c.values().cloned().collect::<Vec<_>>())
        ),
    );
    out.insert("nodes".into(), json!(nodes));
    out.insert("layoutEdges".into(), json!(layout_edges));
    out.insert("mappings".into(), json!(mappings));
    out.insert("diagnostics".into(), model["diagnostics"].clone());
    out.insert("semantic".into(), to(&db.semantic_model()));
    Ok(Value::Object(out))
}

/// Mermaid passes text through DOMPurify, which rewrites markup, so text
/// holding any is not compared. Its DOM ids and click bookkeeping have no
/// counterpart.
fn comparable(value: &Value) -> Value {
    match value {
        Value::String(text) if text.contains(['<', '>', '&']) => json!("(markup)"),
        Value::Array(items) => Value::Array(items.iter().map(comparable).collect()),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(key, _)| *key != "domId" && *key != "haveCallback")
                .map(|(key, value)| (key.clone(), comparable(value)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// What Mermaid's own browser build made of each source in
/// `agentflow-reference.json`: the same sources accepted and rejected, the
/// same model built, and every accepted one drawn.
#[test]
fn agentflow_reference() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mermaid/corpus/agentflow-reference.json");
    let reference: Vec<Value> =
        serde_json::from_str(&fs::read_to_string(&path).expect("corpus file"))
            .expect("corpus JSON");
    assert!(reference.len() > 200);
    assert!(
        reference
            .iter()
            .filter(|r| r.get("model").is_some())
            .count()
            > 150
    );
    let mut failures = Vec::new();
    for entry in &reference {
        let source = entry["src"].as_str().expect("a source");
        let ours = reference_projection(source);
        match (
            entry.get("model"),
            entry.get("error").and_then(Value::as_str),
        ) {
            // Pele's YAML reader has no aliases and takes a lone colon as
            // text, so it accepts these diagrams where Mermaid does not.
            (None, Some(error))
                if error.contains("alias") || error.contains("explicit mapping pair") =>
            {
                if let Err(ours) = &ours {
                    failures.push(format!("{source:?}: mermaid: {error}; ours: {ours}"));
                }
            }
            (None, error) => {
                if ours.is_ok() {
                    failures.push(format!(
                        "{source:?}: mermaid rejects it ({error:?}), ours accepts it"
                    ));
                }
            }
            (Some(model), _) => match &ours {
                Err(error) => failures.push(format!("{source:?}: ours rejects it: {error}")),
                Ok(ours) => {
                    if comparable(ours) != comparable(model) {
                        failures.push(format!(
                            "{source:?}\n  mermaid: {}\n  ours: {}",
                            comparable(model),
                            comparable(ours)
                        ));
                    } else if let Err(error) = render(source, &Options::default(), &Metrics) {
                        failures.push(format!("{source:?}: does not draw: {error}"));
                    }
                }
            },
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
