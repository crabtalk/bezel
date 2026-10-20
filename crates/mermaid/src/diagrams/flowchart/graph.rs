//! Resolves a parsed flowchart into the nodes and edges that are drawn:
//! group membership, collapsed subgraphs, class styles and edge defaults.
//! Pele's `diagrams/flowchart/graph.ts`.

use std::collections::{HashMap, HashSet};

use super::{
    Flowchart,
    db::FlowDb,
    shapes::{IconFrame, NodeShape},
    types::{ArrowType, FlowDir, FlowNode, LabelType, NodeAssets, Stroke, Subgraph},
};
use crate::svg::{
    edges::{Curve, Marker},
    shapes::Shape,
};

type FlowSubgraph = Subgraph<Flowchart>;

#[derive(Clone, Debug, PartialEq)]
pub struct GraphNode {
    pub id: String,
    pub label: Option<String>,
    pub label_type: LabelType,
    pub shape: NodeShape,
    pub is_group: bool,
    pub parent_id: Option<String>,
    pub css_styles: Vec<String>,
    pub css_compiled_styles: Vec<String>,
    pub css_classes: String,
    pub dir: Option<FlowDir>,
    /// Never blank.
    pub link: Option<String>,
    /// Never blank.
    pub link_target: Option<String>,
    /// Never blank.
    pub tooltip: Option<String>,
    pub assets: NodeAssets,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    pub label: String,
    pub label_type: LabelType,
    pub thickness: Option<Stroke>,
    pub minlen: Option<i32>,
    pub arrow_type_start: Option<Marker>,
    pub arrow_type_end: Option<Marker>,
    pub css_compiled_styles: Vec<String>,
    pub style: Vec<String>,
    pub curve: Curve,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlowGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

fn shape_of(vertex: &FlowNode) -> NodeShape {
    if vertex.ext.image().is_some() {
        return NodeShape::Image;
    }
    if vertex.ext.icon().is_some() {
        return NodeShape::Icon(IconFrame::from_form(vertex.ext.form.as_deref()));
    }
    NodeShape::named(vertex.kind.as_deref())
}

/// The markers at the start and the end of a link of `kind`.
fn markers(kind: Option<ArrowType>) -> (Option<Marker>, Option<Marker>) {
    match kind {
        Some(kind) if kind.is_double() => (kind.marker(), kind.marker()),
        Some(kind @ (ArrowType::Point | ArrowType::Circle | ArrowType::Cross)) => {
            (None, kind.marker())
        }
        _ => (None, Some(Marker::Arrow)),
    }
}

fn non_blank(text: Option<&String>) -> Option<String> {
    text.filter(|text| !text.is_empty()).cloned()
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

impl FlowDb {
    /// The nodes and edges drawn, with edges curved as `default_curve` where
    /// neither they nor the diagram name a curve.
    pub fn graph(&self, default_curve: Option<&str>) -> FlowGraph {
        let mut nodes: Vec<GraphNode> = Vec::new();
        let mut edges: Vec<GraphEdge> = Vec::new();
        let subgraphs = &self.subgraphs;

        let mut subgraph_parent: HashMap<&str, &str> = HashMap::new();
        for sg in subgraphs {
            for child in &sg.nodes {
                if self.subgraph(child).is_some() {
                    subgraph_parent.insert(child, &sg.id);
                }
            }
        }

        let is_collapsed = |id: &str| self.subgraph(id).is_some_and(FlowSubgraph::is_collapsed);
        let any_collapsed = subgraphs.iter().any(FlowSubgraph::is_collapsed);
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
            let collapsed = sg.is_collapsed();
            nodes.push(GraphNode {
                id: sg.id.clone(),
                label: Some(sg.title.clone()),
                label_type: sg.label_type,
                parent_id: parent_of.get(sg.id.as_str()).map(|p| p.to_string()),
                css_styles: Vec::new(),
                css_compiled_styles: self.class_styles(&sg.classes),
                css_classes: sg.classes.join(" "),
                shape: NodeShape::Drawn(if collapsed {
                    Shape::CollapsedGroup
                } else {
                    Shape::Rect
                }),
                dir: sg.dir,
                is_group: !collapsed,
                link: None,
                link_target: None,
                tooltip: None,
                assets: NodeAssets::default(),
            });
            by_id.insert(sg.id.clone(), nodes.len() - 1);
        }

        for vertex in self.nodes.values() {
            if hidden.contains(&vertex.id) {
                continue;
            }
            if let Some(&at) = by_id.get(&vertex.id) {
                let existing = &mut nodes[at];
                existing.css_styles = vertex.styles.clone();
                existing.css_compiled_styles = self.class_styles(&vertex.classes);
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
                css_compiled_styles: self.class_styles(&classes),
                css_classes: format!("default {}", vertex.classes.join(" ")),
                dir: None,
                link: non_blank(vertex.link.as_ref()),
                link_target: non_blank(vertex.link_target.as_ref()),
                tooltip: non_blank(self.tooltips.get(&vertex.id)),
                assets: vertex.ext.clone(),
                is_group,
                shape: if is_group {
                    NodeShape::Drawn(Shape::Rect)
                } else {
                    shape_of(vertex)
                },
            });
            by_id.insert(vertex.id.clone(), nodes.len() - 1);
        }

        for (index, raw) in self.edges.iter().enumerate() {
            let start = collapsed_into.get(&raw.start).unwrap_or(&raw.start);
            let end = collapsed_into.get(&raw.end).unwrap_or(&raw.end);
            if start == end
                && (collapsed_into.contains_key(&raw.start)
                    || collapsed_into.contains_key(&raw.end))
            {
                continue;
            }

            let (arrow_start, arrow_end) = markers(raw.kind);
            let mut styles = self.default_edge_style.clone().unwrap_or_default();
            if let Some(style) = &raw.style {
                styles.extend(style.iter().cloned());
            }
            let plain = raw.stroke == Some(Stroke::Invisible) || raw.kind == Some(ArrowType::Open);
            edges.push(GraphEdge {
                id: match raw.id.as_deref().filter(|id| !id.is_empty()) {
                    Some(id) => id.to_string(),
                    None => format!("L_{start}_{end}_{index}"),
                },
                start: start.clone(),
                end: end.clone(),
                label: raw.text.clone(),
                label_type: raw.label_type,
                thickness: raw.stroke,
                minlen: raw.length,
                arrow_type_start: if plain { None } else { arrow_start },
                arrow_type_end: if plain { None } else { arrow_end },
                css_compiled_styles: self.class_styles(&raw.classes),
                style: styles,
                curve: self
                    .curve_of(raw, default_curve)
                    .map_or(Curve::Smooth, |curve| Curve::from_name(&curve)),
            });
        }

        FlowGraph { nodes, edges }
    }
}
