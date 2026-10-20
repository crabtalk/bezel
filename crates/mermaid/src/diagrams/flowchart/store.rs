//! The graph flowchart and agentflow build, and the operations their shared
//! productions perform on it.

use indexmap::{IndexMap, IndexSet};
use serde::Serialize;

use super::types::{
    DocItem, Edge, FlowDir, FlowText, LabelType, LinkInfo, Node, Payload, Subgraph, Vertex,
};
use crate::{
    config::Value,
    diagrams::common::{db::Titles, style_class::StyleClass},
    error::Error,
    js::{self, JsValue},
    util::{chars::unquote, url::safe_url},
};

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreOptions {
    pub inherit_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_edges: Option<usize>,
}

/// The record an id names.
pub enum Element<'a, P: Payload> {
    Edge(&'a mut Edge<P>),
    Node(&'a mut Node<P>),
}

/// A subgraph block reusing an earlier block's id: that block's subgraph,
/// and the direction the repeat gave.
pub struct Repeat<'a, P: Payload> {
    pub subgraph: &'a mut Subgraph<P>,
    pub dir: Option<FlowDir>,
}

/// Nodes, edges, subgraphs and styles, with the bookkeeping Pele serializes
/// beside them.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowStore<P: Payload> {
    pub direction: Option<FlowDir>,
    pub nodes: IndexMap<String, Node<P>>,
    pub edges: Vec<Edge<P>>,
    pub subgraphs: Vec<Subgraph<P>>,
    pub classes: IndexMap<String, StyleClass>,
    pub tooltips: IndexMap<String, String>,
    pub default_edge_style: Option<Vec<String>>,
    pub default_interpolate: Option<String>,
    pub titles: Titles,

    /// Subgraphs by id, as indices into `subgraphs`.
    pub(crate) subgraph_lookup: IndexMap<String, usize>,
    /// Every node a subgraph holds.
    pub(crate) members: IndexSet<String>,
    /// Edges by id, as indices into `edges`.
    pub(crate) edge_by_id: IndexMap<String, usize>,
    pub(crate) pair_count: IndexMap<String, usize>,
    pub(crate) sub_count: usize,
    pub(crate) options: StoreOptions,
}

impl<P: Payload> FlowStore<P> {
    pub fn new(options: StoreOptions) -> Self {
        Self {
            direction: None,
            nodes: IndexMap::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
            classes: IndexMap::new(),
            tooltips: IndexMap::new(),
            default_edge_style: None,
            default_interpolate: None,
            titles: Titles::default(),
            subgraph_lookup: IndexMap::new(),
            members: IndexSet::new(),
            edge_by_id: IndexMap::new(),
            pair_count: IndexMap::new(),
            sub_count: 0,
            options,
        }
    }

    /// Applies `vertex` to the node `id`, created if new. An edge's id names
    /// that edge, and a blank or reserved id nothing; neither creates a node.
    pub fn add_vertex(&mut self, id: &str, vertex: Vertex<'_>) -> Option<Element<'_, P>> {
        if js::trim(id).is_empty() || P::RESERVED.contains(&id) {
            return None;
        }
        if let Some(&at) = self.edge_by_id.get(id) {
            return Some(Element::Edge(&mut self.edges[at]));
        }

        let node = self
            .nodes
            .entry(id.to_string())
            .or_insert_with(|| Node::new(id.to_string()));
        if let Some(label) = vertex.label {
            node.label_type = label.kind;
            node.text = Some(unquote(js::trim(&label.text)).to_string());
        } else if node.text.is_none() {
            node.text = Some(id.to_string());
        }
        if let Some(shape) = vertex.shape {
            node.kind = Some(shape.to_string());
        }
        node.styles.extend_from_slice(vertex.styles);
        let props = node.props.get_or_insert_default();
        if let Some((field, value)) = vertex.prop {
            props.insert(field.to_string(), value.to_string());
        }
        Some(Element::Node(node))
    }

    pub fn add_single_link(
        &mut self,
        start: &str,
        end: &str,
        info: Option<&LinkInfo>,
        id: Option<&str>,
    ) -> Result<(), Error> {
        let mut edge = Edge {
            start: start.to_string(),
            end: end.to_string(),
            kind: None,
            text: String::new(),
            label_type: LabelType::Text,
            stroke: None,
            line: P::Line::default(),
            length: None,
            style: None,
            classes: Vec::new(),
            id: None,
            is_user_defined_id: false,
            interpolate: self.default_interpolate.clone().map(Value::String),
            animate: None,
            animation: None,
            ext: P::Edge::default(),
        };
        if let Some(text) = info.and_then(|info| info.text.as_ref()) {
            edge.text = unquote(js::trim(&text.text)).to_string();
            edge.label_type = text.kind;
        }
        if let Some(info) = info {
            edge.kind = Some(info.kind);
            edge.stroke = Some(info.stroke);
            edge.line = P::line(info);
            edge.length = info
                .length
                .map(|length| if length > 10 { 10 } else { length });
        }
        let pair = format!("{start}\0{end}");
        let existing = self.pair_count.get(&pair).copied().unwrap_or(0);
        let edge_id = match id.filter(|id| !id.is_empty()) {
            Some(id) if !self.edge_by_id.contains_key(id) => {
                edge.is_user_defined_id = true;
                id.to_string()
            }
            _ => {
                let n = if existing == 0 { 0 } else { existing + 1 };
                format!("L_{start}_{end}_{n}")
            }
        };
        edge.id = Some(edge_id.clone());

        if let Some(max) = self.options.max_edges
            && self.edges.len() >= max
        {
            return Err(Error::limit(format!(
                "Edge limit exceeded. {} edges found, but the limit is {max}.",
                self.edges.len()
            ))
            .of(P::KIND));
        }
        self.edges.push(edge);
        self.pair_count.insert(pair, existing + 1);
        if !self.edge_by_id.contains_key(&edge_id) {
            self.edge_by_id.insert(edge_id, self.edges.len() - 1);
        }
        Ok(())
    }

    pub fn add_link(
        &mut self,
        starts: &[String],
        ends: &[String],
        info: Option<&LinkInfo>,
    ) -> Result<(), Error> {
        let id = info
            .and_then(|info| info.id.as_deref())
            .map(|id| id.replacen('@', "", 1));
        let last_start = starts.last();
        let first_end = ends.first();
        for start in starts {
            for end in ends {
                let own = Some(start) == last_start && Some(end) == first_end;
                self.add_single_link(start, end, info, if own { id.as_deref() } else { None })?;
            }
        }
        Ok(())
    }

    /// Mermaid checks the bounds only for a numeric index and crashes on the
    /// strings its own parser passes.
    fn edge_at(&self, pos: &str) -> Result<usize, Error> {
        if let Ok(index) = pos.parse::<usize>()
            && index < self.edges.len()
        {
            return Ok(index);
        }
        Err(Error::semantic(format!(
            "The index {pos} for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and {}. (Help: Ensure that the index is within the range of existing edges.)",
            self.edges.len() as i64 - 1
        ))
        .of(P::KIND))
    }

    /// Adds a subgraph of the nodes `list` names, or adds them to the
    /// subgraph already of that id. A node belongs to the first subgraph that lists it, and a
    /// subgraph never contains itself or a node `free` rejects.
    pub fn add_sub_graph(
        &mut self,
        id: Option<String>,
        list: &[DocItem],
        title_text: Option<&FlowText>,
        free: impl Fn(&str) -> bool,
    ) -> (String, Option<Repeat<'_, P>>) {
        let mut seen: IndexSet<&str> = IndexSet::new();
        let mut node_list: Vec<String> = Vec::new();
        let mut dir: Option<FlowDir> = None;
        for entry in list {
            match entry {
                DocItem::Str(entry) => {
                    if !js::trim(entry).is_empty() && seen.insert(entry.as_str()) {
                        node_list.push(entry.clone());
                    }
                }
                DocItem::List(items) => {
                    for item in items {
                        if !js::trim(item).is_empty() && seen.insert(item.as_str()) {
                            node_list.push(item.clone());
                        }
                    }
                }
                DocItem::Dir(value) => dir = Some(*value),
            }
        }
        if dir.is_none() && self.options.inherit_dir {
            dir = self.direction;
        }

        let id = id.unwrap_or_else(|| format!("subGraph{}", self.sub_count));
        self.sub_count += 1;

        let nodes: Vec<String> = node_list
            .into_iter()
            .filter(|node| *node != id && free(node) && !self.members.contains(node))
            .collect();
        for node in &nodes {
            self.members.insert(node.clone());
        }

        if let Some(&at) = self.subgraph_lookup.get(&id) {
            let subgraph = &mut self.subgraphs[at];
            subgraph.nodes.extend(nodes);
            return (id, Some(Repeat { subgraph, dir }));
        }
        self.subgraphs.push(Subgraph {
            id: id.clone(),
            nodes,
            title: js::trim(title_text.map_or("", |t| t.text.as_str())).to_string(),
            classes: Vec::new(),
            dir,
            label_type: title_text.map_or(LabelType::Markdown, |t| t.kind),
            ext: P::Subgraph::default(),
            metadata: None,
        });
        self.subgraph_lookup
            .insert(id.clone(), self.subgraphs.len() - 1);
        (id, None)
    }

    pub fn subgraph(&self, id: &str) -> Option<&Subgraph<P>> {
        self.subgraph_lookup.get(id).map(|&at| &self.subgraphs[at])
    }

    pub fn subgraph_mut(&mut self, id: &str) -> Option<&mut Subgraph<P>> {
        self.subgraph_lookup
            .get(id)
            .map(|&at| &mut self.subgraphs[at])
    }

    /// `subgraph_lookup`, as Pele serializes it.
    pub(crate) fn subgraphs_by_id(&self) -> IndexMap<&str, &Subgraph<P>> {
        self.subgraph_lookup
            .iter()
            .map(|(id, &at)| (id.as_str(), &self.subgraphs[at]))
            .collect()
    }

    /// `edge_by_id`, as Pele serializes it.
    pub(crate) fn edges_by_id(&self) -> IndexMap<&str, &Edge<P>> {
        self.edge_by_id
            .iter()
            .map(|(id, &at)| (id.as_str(), &self.edges[at]))
            .collect()
    }

    pub fn update_link_interpolate(
        &mut self,
        positions: &[String],
        interpolate: &str,
    ) -> Result<(), Error> {
        for pos in positions {
            if pos == "default" {
                self.default_interpolate = Some(interpolate.to_string());
            } else {
                let at = self.edge_at(pos)?;
                self.edges[at].interpolate = Some(Value::String(interpolate.to_string()));
            }
        }
        Ok(())
    }

    /// Every position gets the one list of styles, which the first edge
    /// position gives a `fill:none` when it sets no fill.
    pub fn update_link(
        &mut self,
        positions: &[String],
        mut style: Vec<String>,
    ) -> Result<(), Error> {
        let mut edges = Vec::new();
        let mut default = false;
        for pos in positions {
            if pos == "default" {
                default = true;
                continue;
            }
            edges.push(self.edge_at(pos)?);
            if !style.is_empty() && !style.iter().any(|s| s.starts_with("fill")) {
                style.push("fill:none".into());
            }
        }
        if default {
            self.default_edge_style = Some(style.clone());
        }
        for at in edges {
            self.edges[at].style = Some(style.clone());
        }
        Ok(())
    }

    pub fn add_class(&mut self, ids: &str, styles: &[String]) {
        let joined = styles
            .join(",")
            .replace("\\,", "§§§")
            .replace(',', ";")
            .replace("§§§", ",");
        let style: Vec<&str> = joined.split(';').collect();
        for id in ids.split(',') {
            let class = self
                .classes
                .entry(id.to_string())
                .or_insert_with(|| StyleClass::new(id));
            for s in &style {
                class.push(s);
            }
        }
    }

    pub fn set_class(&mut self, ids: &str, class_name: &str) {
        for id in ids.split(',') {
            if let Some(node) = self.nodes.get_mut(id) {
                node.classes.push(class_name.to_string());
            }
            if let Some(&at) = self.edge_by_id.get(id) {
                self.edges[at].classes.push(class_name.to_string());
            }
            if let Some(&at) = self.subgraph_lookup.get(id) {
                self.subgraphs[at].classes.push(class_name.to_string());
            }
        }
    }

    pub fn set_tooltip(&mut self, ids: &str, tooltip: &str) {
        for id in ids.split(',') {
            self.tooltips.insert(id.to_string(), tooltip.to_string());
        }
    }

    pub fn set_link(&mut self, ids: &str, link: &str, target: Option<&str>) {
        let url = js::trim(link);
        for id in ids.split(',') {
            if let Some(vertex) = self.nodes.get_mut(id) {
                vertex.link = (!url.is_empty()).then(|| safe_url(url));
                vertex.link_target = target.map(str::to_string);
            }
        }
        self.set_class(ids, "clickable");
    }

    /// Callbacks are never executed; the node is only marked clickable, as
    /// in Mermaid's strict mode.
    pub fn set_click_event(&mut self, ids: &str) {
        self.set_class(ids, "clickable");
    }

    /// The styles of the classes `names` that are defined, in order, each
    /// trimmed.
    pub fn class_styles(&self, names: &[String]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for name in names {
            if let Some(def) = self.classes.get(name) {
                out.extend(
                    def.styles
                        .iter()
                        .chain(&def.text_styles)
                        .map(|s| js::trim(s).to_string()),
                );
            }
        }
        out
    }

    /// The curve `edge` is drawn with: its own, the diagram's, or
    /// `default_curve`, the first not blank. `None` where the edge names it
    /// with something other than a string.
    pub fn curve_of(&self, edge: &Edge<P>, default_curve: Option<&str>) -> Option<String> {
        match edge.interpolate.as_ref().filter(|v| v.truthy()) {
            Some(value) => value.as_str().map(str::to_string),
            None => self
                .default_interpolate
                .clone()
                .filter(|curve| !curve.is_empty())
                .or_else(|| default_curve.filter(|c| !c.is_empty()).map(str::to_string)),
        }
    }
}
