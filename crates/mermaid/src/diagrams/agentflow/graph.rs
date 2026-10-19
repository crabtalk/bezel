//! Resolves a parsed agentflow into the nodes and edges that are drawn: flow
//! membership, collapsed flows, shapes reduced to the ones agentflow has,
//! class styles and palette slots. Shape and containment problems are
//! reported on the model as diagnostics. Pele's `diagrams/agentflow/graph.ts`.

use indexmap::{IndexMap, IndexSet};

use super::{
    color_slots::assign_color_slots,
    db::AgentflowDb,
    shapes::{normalise_node_shapes, resolve_shape_alias},
    types::{DiagnosticId, LabelType, Metadata},
};
use crate::config::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct GraphNode {
    pub id: String,
    pub label: Option<String>,
    pub label_type: LabelType,
    pub shape: String,
    pub is_group: bool,
    pub parent_id: Option<String>,
    pub css_styles: Vec<String>,
    pub css_compiled_styles: Vec<String>,
    pub css_classes: String,
    pub dir: Option<String>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub tooltip: Option<String>,
    /// Palette slot of a container.
    pub color_index: Option<usize>,
    /// What a node is, which picks its palette slot.
    pub kind: Option<&'static str>,
    pub metadata: Option<Metadata>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphEdge {
    pub id: String,
    pub is_user_defined_id: bool,
    pub start: String,
    pub end: String,
    pub kind: String,
    pub label: String,
    pub label_type: LabelType,
    pub thickness: Option<String>,
    pub pattern: Option<String>,
    pub minlen: Option<i32>,
    pub arrow_type_start: String,
    pub arrow_type_end: String,
    pub css_compiled_styles: Vec<String>,
    pub style: Vec<String>,
    pub classes: String,
    pub animate: Option<Value>,
    pub animation: Option<Value>,
    /// The curve's name; `None` also where the edge names it with something
    /// other than a string.
    pub curve: Option<String>,
    pub metadata: Option<Metadata>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgentGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

fn compiled(db: &AgentflowDb, class_names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in class_names {
        if let Some(def) = db.classes.get(name) {
            out.extend(def.styles.iter().cloned());
            out = out
                .iter()
                .map(|s| crate::js::trim(s).to_string())
                .collect();
            out.extend(def.text_styles.iter().cloned());
            out = out
                .iter()
                .map(|s| crate::js::trim(s).to_string())
                .collect();
        }
    }
    out
}

fn is_collapsed(metadata: Option<&Metadata>) -> bool {
    metadata.and_then(|m| m.get("view")).and_then(Value::as_str) == Some("collapsed")
}

pub fn build_agent_graph(
    db: &mut AgentflowDb,
    palette_length: usize,
    default_curve: Option<&str>,
) -> AgentGraph {
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let count = db.subgraphs.len();

    // Everything inside a collapsed flow is hidden, and stands for its
    // outermost collapsed flow. Mermaid walks the whole inside of each
    // collapsed flow, in the order the flows closed, and the last walk to
    // reach a thing wins. A thing has one container, so a later walk can only
    // enter an earlier one through its root: it stops there, and the root's
    // owner is followed afterwards.
    let mut hidden: IndexSet<String> = IndexSet::new();
    let mut owner: IndexMap<String, String> = IndexMap::new();
    let mut walked: IndexSet<String> = IndexSet::new();
    for sg in &db.subgraphs {
        if !is_collapsed(sg.metadata.as_ref()) || hidden.contains(&sg.id) {
            continue;
        }
        let mut pending = vec![sg.id.clone()];
        while let Some(next) = pending.pop() {
            let Some(members) = db.subgraph(&next).map(|s| &s.nodes) else {
                continue;
            };
            for child in members {
                if *child == sg.id || hidden.contains(child) {
                    continue;
                }
                hidden.insert(child.clone());
                owner.insert(child.clone(), sg.id.clone());
                if !walked.contains(child) {
                    pending.push(child.clone());
                }
            }
        }
        walked.insert(sg.id.clone());
    }
    let mut collapsed_into: IndexMap<String, String> = IndexMap::new();
    for id in &hidden {
        let mut path = vec![id.clone()];
        let mut top = owner[id].clone();
        loop {
            let known = collapsed_into.get(&top).cloned();
            let up = known.clone().or_else(|| owner.get(&top).cloned());
            let Some(up) = up else {
                break;
            };
            if known.is_none() {
                path.push(top.clone());
            }
            top = up;
            if known.is_some() {
                break;
            }
        }
        for p in path {
            collapsed_into.insert(p, top.clone());
        }
    }

    let position: IndexMap<&str, usize> = db
        .subgraphs
        .iter()
        .enumerate()
        .map(|(i, sg)| (sg.id.as_str(), i))
        .collect();

    // Two flows can each name the other. The nesting that would close the
    // loop is dropped.
    let mut parent_of: IndexMap<String, String> = IndexMap::new();
    let mut violations: Vec<(String, String)> = Vec::new();
    for i in (0..count).rev() {
        let sg = &db.subgraphs[i];
        if hidden.contains(&sg.id) {
            continue;
        }
        for id in &sg.nodes {
            // Only a flow handled earlier in this pass can already be above
            // the parent.
            let closes_loop = match position.get(id.as_str()) {
                Some(&child_at) if child_at >= i => {
                    let mut current: Option<&str> = Some(&sg.id);
                    let mut hops = 0;
                    let mut found = false;
                    while let Some(at) = current
                        && hops <= count
                    {
                        if at == id {
                            found = true;
                            break;
                        }
                        current = parent_of.get(at).map(String::as_str);
                        hops += 1;
                    }
                    found
                }
                _ => false,
            };
            if closes_loop {
                violations.push((sg.id.clone(), id.clone()));
                continue;
            }
            parent_of.insert(id.clone(), sg.id.clone());
        }
    }
    drop(position);
    for (container, id) in violations {
        db.emit_warning(
            DiagnosticId::ContainmentViolation,
            format!(
                "Container \"{container}\" cannot contain \"{id}\" because \"{id}\" already contains it. The nesting that would close the loop is dropped."
            ),
            &id,
        );
    }

    let mut by_id: IndexMap<String, usize> = IndexMap::new();
    for i in (0..count).rev() {
        let sg = &db.subgraphs[i];
        if hidden.contains(&sg.id) {
            continue;
        }
        let collapsed = is_collapsed(sg.metadata.as_ref());
        let metadata = if collapsed {
            let mut metadata = sg.metadata.clone().unwrap_or_default();
            metadata.insert("containerType", Value::String(sg.kind.to_string()));
            Some(metadata)
        } else {
            sg.metadata.clone()
        };
        nodes.push(GraphNode {
            id: sg.id.clone(),
            label: Some(sg.title.clone()),
            label_type: sg.label_type,
            parent_id: parent_of.get(&sg.id).cloned(),
            css_styles: Vec::new(),
            css_compiled_styles: compiled(db, &sg.classes),
            css_classes: sg.classes.join(" "),
            shape: if collapsed {
                "collapsedGroup".into()
            } else {
                "flowGroup".into()
            },
            dir: sg.dir.clone(),
            is_group: !collapsed,
            link: None,
            link_target: None,
            tooltip: None,
            color_index: None,
            kind: None,
            metadata,
        });
        by_id.insert(sg.id.clone(), nodes.len() - 1);
    }

    for vertex in db.nodes.values() {
        if hidden.contains(&vertex.id) {
            continue;
        }
        if let Some(&at) = by_id.get(&vertex.id) {
            let existing = &mut nodes[at];
            existing.css_styles = vertex.styles.clone();
            existing.css_compiled_styles = compiled(db, &vertex.classes);
            existing.css_classes = vertex.classes.join(" ");
            continue;
        }
        let kind = resolve_shape_alias(vertex.kind.as_deref());
        let mut classes = vec!["default".to_string(), "node".to_string()];
        classes.extend(vertex.classes.iter().cloned());
        nodes.push(GraphNode {
            id: vertex.id.clone(),
            label: vertex.text.clone(),
            label_type: vertex.label_type,
            parent_id: parent_of.get(&vertex.id).cloned(),
            css_styles: vertex.styles.clone(),
            css_compiled_styles: compiled(db, &classes),
            css_classes: format!("default {}", vertex.classes.join(" ")),
            dir: vertex.dir.clone(),
            link: vertex.link.clone(),
            link_target: vertex.link_target.clone(),
            tooltip: db.tooltips.get(&vertex.id).cloned(),
            metadata: vertex.metadata.clone(),
            is_group: false,
            shape: match kind {
                None | Some("square") => "squareRect".into(),
                Some(kind) => kind.to_string(),
            },
            color_index: None,
            kind: None,
        });
        by_id.insert(vertex.id.clone(), nodes.len() - 1);
    }

    for (index, raw) in db.edges.iter().enumerate() {
        let start = collapsed_into.get(&raw.start).unwrap_or(&raw.start).clone();
        let end = collapsed_into.get(&raw.end).unwrap_or(&raw.end).clone();
        // An edge between two things inside one collapsed flow has nothing
        // left to connect.
        if start == end && raw.start != raw.end {
            continue;
        }
        let mut styles = db.default_edge_style.clone().unwrap_or_default();
        if let Some(style) = &raw.style {
            styles.extend(style.iter().cloned());
        }
        let arrow = match raw.kind.as_deref() {
            Some("arrow_open") => "none",
            Some(kind @ ("arrow_cross" | "arrow_point")) => kind,
            _ => "arrow_point",
        };
        // `raw.interpolate || db.defaultInterpolate || defaultCurve`.
        let curve = match raw.interpolate.as_ref().filter(|v| v.truthy()) {
            Some(value) => value.as_str().map(str::to_string),
            None => db
                .default_interpolate
                .clone()
                .filter(|curve| !curve.is_empty())
                .or_else(|| default_curve.filter(|c| !c.is_empty()).map(str::to_string)),
        };
        edges.push(GraphEdge {
            id: match raw.id.as_deref().filter(|id| !id.is_empty()) {
                Some(id) => id.to_string(),
                None => format!("L_{start}_{end}_{index}"),
            },
            is_user_defined_id: raw.is_user_defined_id,
            kind: raw.kind.clone().unwrap_or_else(|| "normal".into()),
            label: raw.text.clone(),
            label_type: raw.label_type,
            thickness: raw.stroke.clone(),
            pattern: raw.stroke.clone(),
            minlen: raw.length,
            classes: "edge-thickness-normal edge-pattern-solid flowchart-link".into(),
            arrow_type_start: "none".into(),
            arrow_type_end: arrow.into(),
            css_compiled_styles: compiled(db, &raw.classes),
            style: styles,
            animate: raw.animate.clone(),
            animation: raw.animation.clone(),
            curve,
            metadata: raw.metadata.clone(),
            start,
            end,
        });
    }

    normalise_node_shapes(&mut nodes, db);

    // Containers are numbered in the order they were written: a walk of the
    // nesting, parents first. `subgraphs` itself is in the order the blocks
    // closed, with a nested flow before its parent.
    let mut children: IndexMap<String, Vec<String>> = IndexMap::new();
    for sg in &db.subgraphs {
        let Some(parent) = parent_of.get(&sg.id) else {
            continue;
        };
        children
            .entry(parent.clone())
            .or_default()
            .push(sg.id.clone());
    }
    let mut order: IndexMap<String, usize> = IndexMap::new();
    for sg in &db.subgraphs {
        if parent_of.contains_key(&sg.id) {
            continue;
        }
        let mut pending = vec![sg.id.clone()];
        while let Some(id) = pending.pop() {
            if order.contains_key(&id) {
                continue;
            }
            let n = order.len();
            order.insert(id.clone(), n);
            if let Some(kids) = children.get(&id) {
                pending.extend(kids.iter().rev().cloned());
            }
        }
    }

    let db: &AgentflowDb = db;
    assign_color_slots(
        &mut nodes,
        |id| {
            if db.connectors.contains(id) {
                return Some("connector");
            }
            db.nodes.get(id).map(|vertex| db.vertex_kind(vertex).name())
        },
        &order,
        palette_length,
    );

    AgentGraph { nodes, edges }
}
