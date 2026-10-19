//! Resolves a parsed flowchart into the nodes and edges that are drawn:
//! group membership, collapsed subgraphs, class styles and edge defaults.
//! Pele's `diagrams/flowchart/graph.ts`.

use std::collections::{HashMap, HashSet};

use super::{
    db::FlowDb,
    types::{FlowNode, LabelType},
};
use crate::config::{Map, Value};

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
    pub icon: Option<String>,
    pub pos: Option<String>,
    pub img: Option<String>,
    pub asset_width: Option<f64>,
    pub asset_height: Option<f64>,
    pub constraint: Option<String>,
    pub color_index: Option<usize>,
    pub metadata: Option<Map>,
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
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlowGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

fn shape_of(vertex: &FlowNode) -> String {
    if vertex.img.as_deref().is_some_and(|img| !img.is_empty()) {
        return "imageSquare".into();
    }
    if vertex.icon.as_deref().is_some_and(|icon| !icon.is_empty()) {
        return match vertex.form.as_deref() {
            Some("circle") => "iconCircle",
            Some("square") => "iconSquare",
            Some("rounded") => "iconRounded",
            _ => "icon",
        }
        .into();
    }
    match vertex.kind.as_deref() {
        Some("square") | None => "squareRect".into(),
        Some("round") => "roundedRect".into(),
        Some(kind) => kind.into(),
    }
}

fn arrows(kind: Option<&str>) -> (String, String) {
    match kind {
        Some(kind @ ("arrow_point" | "arrow_circle" | "arrow_cross")) => {
            ("none".into(), kind.into())
        }
        Some(kind @ ("double_arrow_point" | "double_arrow_circle" | "double_arrow_cross")) => {
            let single = kind.replacen("double_", "", 1);
            (single.clone(), single)
        }
        _ => ("none".into(), "arrow_point".into()),
    }
}

fn collapsed(metadata: Option<&Map>) -> bool {
    metadata
        .and_then(|metadata| metadata.get("view"))
        .and_then(Value::as_str)
        == Some("collapsed")
}

/// The outermost collapsed subgraph around subgraph `id`, resolved once per
/// chain of parents.
fn outermost_collapsed<'m>(
    id: &'m str,
    outermost: &mut HashMap<&'m str, Option<&'m str>>,
    subgraph_parent: &HashMap<&'m str, &'m str>,
    is_collapsed: &dyn Fn(&str) -> bool,
) -> Option<&'m str> {
    let mut chain: Vec<&str> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut current = Some(id);
    while let Some(at) = current {
        if outermost.contains_key(at) || seen.contains(at) {
            break;
        }
        seen.insert(at);
        chain.push(at);
        current = subgraph_parent.get(at).copied();
    }
    let mut result = current.and_then(|at| outermost.get(at).copied().flatten());
    for &link in chain.iter().rev() {
        if result.is_none() && is_collapsed(link) {
            result = Some(link);
        }
        outermost.insert(link, result);
    }
    result
}

pub fn build_flow_graph(model: &FlowDb, default_curve: Option<&str>) -> FlowGraph {
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let subgraphs = &model.subgraphs;

    let compiled = |class_names: &[String]| -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for name in class_names {
            if let Some(def) = model.classes.get(name) {
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
    };

    let mut subgraph_parent: HashMap<&str, &str> = HashMap::new();
    for sg in subgraphs {
        for child in &sg.nodes {
            if model.subgraph(child).is_some() {
                subgraph_parent.insert(child, &sg.id);
            }
        }
    }

    let mut declaration_index: HashMap<&str, usize> = HashMap::new();
    let mut children_of: HashMap<&str, Vec<&str>> = HashMap::new();
    for sg in subgraphs {
        let Some(&parent) = subgraph_parent.get(sg.id.as_str()) else {
            continue;
        };
        children_of.entry(parent).or_default().push(&sg.id);
    }
    let mut next_index = 0;
    for sg in subgraphs {
        if subgraph_parent.contains_key(sg.id.as_str()) {
            continue;
        }
        let mut pending: Vec<&str> = vec![&sg.id];
        while let Some(id) = pending.pop() {
            if declaration_index.contains_key(id) {
                continue;
            }
            declaration_index.insert(id, next_index);
            next_index += 1;
            if let Some(kids) = children_of.get(id) {
                pending.extend(kids.iter().rev());
            }
        }
    }

    let is_collapsed = |id: &str| collapsed(model.subgraph(id).and_then(|sg| sg.metadata.as_ref()));
    let any_collapsed = subgraphs.iter().any(|sg| collapsed(sg.metadata.as_ref()));
    let mut outermost: HashMap<&str, Option<&str>> = HashMap::new();
    let mut hidden: HashSet<String> = HashSet::new();
    let mut collapsed_into: HashMap<String, String> = HashMap::new();
    if any_collapsed {
        for sg in subgraphs {
            let Some(ancestor) =
                outermost_collapsed(&sg.id, &mut outermost, &subgraph_parent, &is_collapsed)
            else {
                continue;
            };
            if sg.id != ancestor {
                hidden.insert(sg.id.clone());
                collapsed_into.insert(sg.id.clone(), ancestor.to_string());
            }
            for child in &sg.nodes {
                if *child == ancestor {
                    continue;
                }
                hidden.insert(child.clone());
                collapsed_into.insert(child.clone(), ancestor.to_string());
            }
        }
    }

    let mut parent_of: HashMap<&str, &str> = HashMap::new();
    let mut groups: HashSet<&str> = HashSet::new();
    for sg in subgraphs.iter().rev() {
        if hidden.contains(&sg.id) {
            continue;
        }
        if !sg.nodes.is_empty() {
            groups.insert(&sg.id);
        }
        for id in &sg.nodes {
            parent_of.insert(id, &sg.id);
        }
    }

    let mut by_id: HashMap<String, usize> = HashMap::new();
    for sg in subgraphs.iter().rev() {
        if hidden.contains(&sg.id) {
            continue;
        }
        let collapsed = collapsed(sg.metadata.as_ref());
        nodes.push(GraphNode {
            id: sg.id.clone(),
            label: Some(sg.title.clone()),
            label_type: sg.label_type,
            parent_id: parent_of.get(sg.id.as_str()).map(|p| p.to_string()),
            css_styles: Vec::new(),
            css_compiled_styles: compiled(&sg.classes),
            css_classes: sg.classes.join(" "),
            shape: if collapsed { "collapsedGroup" } else { "rect" }.into(),
            dir: sg.dir.clone(),
            is_group: !collapsed,
            color_index: declaration_index.get(sg.id.as_str()).copied(),
            metadata: if collapsed { None } else { sg.metadata.clone() },
            link: None,
            link_target: None,
            tooltip: None,
            icon: None,
            pos: None,
            img: None,
            asset_width: None,
            asset_height: None,
            constraint: None,
        });
        by_id.insert(sg.id.clone(), nodes.len() - 1);
    }

    for vertex in model.nodes.values() {
        if hidden.contains(&vertex.id) {
            continue;
        }
        if let Some(&at) = by_id.get(&vertex.id) {
            let existing = &mut nodes[at];
            existing.css_styles = vertex.styles.clone();
            existing.css_compiled_styles = compiled(&vertex.classes);
            existing.css_classes = vertex.classes.join(" ");
            continue;
        }
        let is_group = groups.contains(vertex.id.as_str());
        let mut classes = vec!["default".to_string(), "node".to_string()];
        classes.extend(vertex.classes.iter().cloned());
        nodes.push(GraphNode {
            id: vertex.id.clone(),
            label: vertex.text.clone(),
            label_type: vertex.label_type,
            parent_id: parent_of.get(vertex.id.as_str()).map(|p| p.to_string()),
            css_styles: vertex.styles.clone(),
            css_compiled_styles: compiled(&classes),
            css_classes: format!("default {}", vertex.classes.join(" ")),
            dir: vertex.dir.clone(),
            link: vertex.link.clone(),
            link_target: vertex.link_target.clone(),
            tooltip: model.tooltips.get(&vertex.id).cloned(),
            icon: vertex.icon.clone(),
            pos: vertex.pos.clone(),
            img: vertex.img.clone(),
            asset_width: vertex.asset_width,
            asset_height: vertex.asset_height,
            constraint: vertex.constraint.clone(),
            is_group,
            shape: if is_group {
                "rect".into()
            } else {
                shape_of(vertex)
            },
            color_index: None,
            metadata: None,
        });
        by_id.insert(vertex.id.clone(), nodes.len() - 1);
    }

    for (index, raw) in model.edges.iter().enumerate() {
        let start = collapsed_into.get(&raw.start).unwrap_or(&raw.start);
        let end = collapsed_into.get(&raw.end).unwrap_or(&raw.end);
        if start == end
            && (collapsed_into.contains_key(&raw.start) || collapsed_into.contains_key(&raw.end))
        {
            continue;
        }

        let (arrow_start, arrow_end) = arrows(raw.kind.as_deref());
        let mut styles = model.default_edge_style.clone().unwrap_or_default();
        if let Some(style) = &raw.style {
            styles.extend(style.iter().cloned());
        }
        let plain =
            raw.stroke.as_deref() == Some("invisible") || raw.kind.as_deref() == Some("arrow_open");
        // `raw.interpolate || model.defaultInterpolate || defaultCurve`.
        let curve = match raw.interpolate.as_ref().filter(|v| v.truthy()) {
            Some(value) => value.as_str().map(str::to_string),
            None => model
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
            start: start.clone(),
            end: end.clone(),
            kind: raw.kind.clone().unwrap_or_else(|| "normal".into()),
            label: raw.text.clone(),
            label_type: raw.label_type,
            thickness: raw.stroke.clone(),
            pattern: raw.stroke.clone(),
            minlen: raw.length,
            classes: if raw.stroke.as_deref() == Some("invisible") {
                String::new()
            } else {
                "edge-thickness-normal edge-pattern-solid flowchart-link".into()
            },
            arrow_type_start: if plain { "none".into() } else { arrow_start },
            arrow_type_end: if plain { "none".into() } else { arrow_end },
            css_compiled_styles: compiled(&raw.classes),
            style: styles,
            animate: raw.animate.clone(),
            animation: raw.animation.clone(),
            curve,
        });
    }

    FlowGraph { nodes, edges }
}
