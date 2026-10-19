//! What the flowchart parser builds. Pele's `diagrams/flowchart/db.ts`.

use indexmap::{IndexMap, IndexSet};
use serde::{Serialize, Serializer, ser::SerializeMap};

use super::{
    shapes::is_valid_shape,
    types::{
        DocItem, FlowClassDef, FlowEdge, FlowNode, FlowSubgraph, FlowText, LabelType, LinkInfo,
    },
};
use crate::{
    config::{Map, Value},
    detect::Kind,
    error::Error,
    js::{self, Lazy},
    util::url::safe_url,
    yaml::parse_yaml,
};

static RE_LEAD: Lazy = Lazy::new(r"^\s+", "g");
static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");
static RE_SPACE: Lazy = Lazy::new(r"\s", "");

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowDbOptions {
    pub inherit_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_edges: Option<usize>,
}

fn label_type(kind: Option<&str>) -> LabelType {
    match kind {
        Some("text") => LabelType::Text,
        Some("string") => LabelType::String,
        _ => LabelType::Markdown,
    }
}

fn unquote(text: &str) -> &str {
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        &text[1..text.len() - 1]
    } else {
        text
    }
}

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

/// The flowchart model. Serializes as Pele's `FlowDb` does through
/// `JSON.stringify`, its bookkeeping included.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowDb {
    pub direction: Option<String>,
    pub nodes: IndexMap<String, FlowNode>,
    pub edges: Vec<FlowEdge>,
    pub subgraphs: Vec<FlowSubgraph>,
    pub classes: IndexMap<String, FlowClassDef>,
    pub tooltips: IndexMap<String, String>,
    pub default_edge_style: Option<Vec<String>>,
    pub default_interpolate: Option<String>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,

    /// Subgraphs by id, as indices into `subgraphs`.
    subgraph_lookup: IndexMap<String, usize>,
    members: IndexSet<String>,
    /// Edges by id, as indices into `edges`.
    edge_by_id: IndexMap<String, usize>,
    pair_count: IndexMap<String, usize>,
    sub_count: usize,
    options: FlowDbOptions,
}

impl FlowDb {
    pub fn new(options: FlowDbOptions) -> Self {
        Self {
            direction: None,
            nodes: IndexMap::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
            classes: IndexMap::new(),
            tooltips: IndexMap::new(),
            default_edge_style: None,
            default_interpolate: None,
            title: None,
            acc_title: None,
            acc_descr: None,
            subgraph_lookup: IndexMap::new(),
            members: IndexSet::new(),
            edge_by_id: IndexMap::new(),
            pair_count: IndexMap::new(),
            sub_count: 0,
            options,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_vertex(
        &mut self,
        id: &str,
        text_obj: Option<&FlowText>,
        kind: Option<&str>,
        style: Option<&[String]>,
        classes: Option<&[String]>,
        dir: Option<&str>,
        props: Option<IndexMap<String, String>>,
        metadata: Option<&str>,
    ) -> Result<(), Error> {
        if id.is_empty() || js::trim(id).is_empty() {
            return Ok(());
        }

        let mut doc: Option<Map> = None;
        if let Some(metadata) = metadata {
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
            if let Value::Map(map) = parsed {
                doc = Some(map);
            }
        }

        if let Some(&at) = self.subgraph_lookup.get(id)
            && let Some(doc) = &doc
        {
            let subgraph = &mut self.subgraphs[at];
            let mut merged = subgraph.metadata.take().unwrap_or_default();
            for (key, value) in doc.iter() {
                merged.insert(key, value.clone());
            }
            subgraph.metadata = Some(merged);
            return Ok(());
        }

        if let Some(&at) = self.edge_by_id.get(id) {
            let edge = &mut self.edges[at];
            if let Some(doc) = &doc {
                if let Some(value) = doc.get("animate") {
                    edge.animate = Some(value.clone());
                }
                if let Some(value) = doc.get("animation") {
                    edge.animation = Some(value.clone());
                }
                if let Some(value) = doc.get("curve") {
                    edge.interpolate = Some(value.clone());
                }
            }
            return Ok(());
        }

        let vertex = self
            .nodes
            .entry(id.to_string())
            .or_insert_with(|| FlowNode {
                id: id.to_string(),
                text: None,
                label_type: LabelType::Text,
                kind: None,
                styles: Vec::new(),
                classes: Vec::new(),
                dir: None,
                props: None,
                link: None,
                link_target: None,
                icon: None,
                form: None,
                pos: None,
                img: None,
                constraint: None,
                asset_width: None,
                asset_height: None,
            });

        if let Some(text_obj) = text_obj {
            vertex.label_type = text_obj.kind;
            vertex.text = Some(unquote(js::trim(&text_obj.text)).to_string());
        } else if vertex.text.is_none() {
            vertex.text = Some(id.to_string());
        }
        if let Some(kind) = kind {
            vertex.kind = Some(kind.to_string());
        }
        if let Some(style) = style {
            vertex.styles.extend(style.iter().cloned());
        }
        if let Some(classes) = classes {
            vertex.classes.extend(classes.iter().cloned());
        }
        if let Some(dir) = dir {
            vertex.dir = Some(dir.to_string());
        }
        let props = props.unwrap_or_default();
        match &mut vertex.props {
            None => vertex.props = Some(props),
            Some(existing) => existing.extend(props),
        }

        let Some(doc) = doc else {
            return Ok(());
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
            vertex.kind = Some(shape);
        }
        let label = doc.get("label").and_then(Value::as_str);
        let blank_label = label.is_none_or(|label| js::trim(label).is_empty());
        if let Some(value) = doc.get("label").filter(|v| v.truthy()) {
            vertex.text = Some(value.to_js_string());
            vertex.label_type = label_type(doc.get("labelType").and_then(Value::as_str));
        }
        if let Some(icon) = doc.get("icon").filter(|v| v.truthy()) {
            vertex.icon = Some(icon.to_js_string());
            if blank_label && vertex.text.as_deref() == Some(id) {
                vertex.text = Some(String::new());
            }
        }
        if let Some(form) = doc.get("form").filter(|v| v.truthy()) {
            vertex.form = Some(form.to_js_string());
        }
        if let Some(pos) = doc.get("pos").filter(|v| v.truthy()) {
            vertex.pos = Some(pos.to_js_string());
        }
        if let Some(img) = doc.get("img").filter(|v| v.truthy()) {
            vertex.img = Some(img.to_js_string());
            if blank_label && vertex.text.as_deref() == Some(id) {
                vertex.text = Some(String::new());
            }
        }
        if let Some(constraint) = doc.get("constraint").filter(|v| v.truthy()) {
            vertex.constraint = Some(constraint.to_js_string());
        }
        if let Some(w) = doc.get("w").filter(|v| v.truthy()) {
            vertex.asset_width = Some(w.to_number());
        }
        if let Some(h) = doc.get("h").filter(|v| v.truthy()) {
            vertex.asset_height = Some(h.to_number());
        }
        Ok(())
    }

    pub fn add_single_link(
        &mut self,
        start: &str,
        end: &str,
        info: Option<&LinkInfo>,
        id: Option<&str>,
    ) -> Result<(), Error> {
        let mut edge = FlowEdge {
            start: start.to_string(),
            end: end.to_string(),
            kind: None,
            text: String::new(),
            label_type: LabelType::Text,
            stroke: None,
            length: None,
            style: None,
            classes: Vec::new(),
            id: None,
            is_user_defined_id: false,
            interpolate: self.default_interpolate.clone().map(Value::String),
            animate: None,
            animation: None,
        };
        if let Some(text) = info.and_then(|info| info.text.as_ref()) {
            edge.text = unquote(js::trim(&text.text)).to_string();
            edge.label_type = text.kind;
        }
        if let Some(info) = info {
            edge.kind = Some(info.kind.clone());
            edge.stroke = Some(info.stroke.clone());
            edge.length = info
                .length
                .map(|length| if length > 10 { 10 } else { length });
        }
        let pair = format!("{start}\0{end}");
        let edge_id = match id.filter(|id| !id.is_empty()) {
            Some(id) if !self.edge_by_id.contains_key(id) => {
                edge.is_user_defined_id = true;
                id.to_string()
            }
            _ => {
                let existing = self.pair_count.get(&pair).copied().unwrap_or(0);
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
            .of(Kind::Flowchart));
        }
        self.edges.push(edge);
        *self.pair_count.entry(pair).or_insert(0) += 1;
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

    fn edge_at(&self, pos: &str) -> Result<usize, Error> {
        let index = js::number(pos);
        if index >= 0.0 && index.fract() == 0.0 && (index as usize) < self.edges.len() {
            return Ok(index as usize);
        }
        Err(Error::semantic(format!(
            "The index {pos} for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and {}. (Help: Ensure that the index is within the range of existing edges.)",
            self.edges.len() as i64 - 1
        ))
        .of(Kind::Flowchart))
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
                .or_insert_with(|| FlowClassDef {
                    id: id.to_string(),
                    styles: Vec::new(),
                    text_styles: Vec::new(),
                });
            for s in &style {
                if s.contains("color") {
                    class.text_styles.push(s.replacen("fill", "bgFill", 1));
                }
                class.styles.push(s.to_string());
            }
        }
    }

    pub fn set_direction(&mut self, dir: &str) {
        let mut d = js::trim(dir);
        if d.contains('<') {
            d = "RL";
        }
        if d.contains('^') {
            d = "BT";
        }
        if d.contains('>') {
            d = "LR";
        }
        if d.contains('v') {
            d = "TB";
        }
        if d == "TD" {
            d = "TB";
        }
        self.direction = Some(d.to_string());
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

    pub fn set_tooltip(&mut self, ids: &str, tooltip: Option<&str>) {
        let Some(tooltip) = tooltip else {
            return;
        };
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

    pub fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(RE_LEAD.replace(text, |_| String::new()));
    }

    pub fn set_acc_description(&mut self, text: &str) {
        self.acc_descr = Some(RE_INDENT.replace(text, |_| "\n".into()));
    }

    /// `same` when the title is the id's own text, as it is for a subgraph
    /// without a bracketed title.
    pub fn add_sub_graph(
        &mut self,
        id_text: Option<&FlowText>,
        list: &[DocItem],
        title_text: Option<&FlowText>,
        same: bool,
    ) -> String {
        let mut id: Option<String> = id_text.map(|t| js::trim(&t.text).to_string());
        let title = title_text.map_or("", |t| t.text.as_str());
        if id_text.is_some() && same && RE_SPACE.test(title) {
            id = None;
        }

        let mut seen: IndexSet<&str> = IndexSet::new();
        let mut node_list: Vec<String> = Vec::new();
        let mut dir: Option<String> = None;
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
                DocItem::Dir(value) => dir = Some(value.clone()),
            }
        }
        if dir.is_none() && self.options.inherit_dir {
            dir = self.direction.clone();
        }

        let id = id.unwrap_or_else(|| format!("subGraph{}", self.sub_count));
        self.sub_count += 1;

        // A node belongs to the first subgraph that lists it, and a subgraph
        // never contains itself.
        let nodes: Vec<String> = node_list
            .into_iter()
            .filter(|node| !self.members.contains(node) && *node != id)
            .collect();
        for node in &nodes {
            self.members.insert(node.clone());
        }

        match self.subgraph_lookup.get(&id) {
            Some(&at) => self.subgraphs[at].nodes.extend(nodes),
            None => {
                self.subgraphs.push(FlowSubgraph {
                    id: id.clone(),
                    nodes,
                    title: js::trim(title).to_string(),
                    classes: Vec::new(),
                    dir,
                    label_type: title_text.map_or(LabelType::Markdown, |t| t.kind),
                    metadata: None,
                });
                self.subgraph_lookup
                    .insert(id.clone(), self.subgraphs.len() - 1);
            }
        }
        id
    }

    pub fn destruct_link(&self, end: &str, start: Option<&str>) -> LinkInfo {
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

    pub fn subgraph(&self, id: &str) -> Option<&FlowSubgraph> {
        self.subgraph_lookup.get(id).map(|&at| &self.subgraphs[at])
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
        let lookup: IndexMap<&str, &FlowSubgraph> = self
            .subgraph_lookup
            .iter()
            .map(|(id, &at)| (id.as_str(), &self.subgraphs[at]))
            .collect();
        map.serialize_entry("subgraphLookup", &lookup)?;
        map.serialize_entry("members", &self.members)?;
        let by_id: IndexMap<&str, &FlowEdge> = self
            .edge_by_id
            .iter()
            .map(|(id, &at)| (id.as_str(), &self.edges[at]))
            .collect();
        map.serialize_entry("edgeById", &by_id)?;
        map.serialize_entry("pairCount", &self.pair_count)?;
        map.serialize_entry("subCount", &self.sub_count)?;
        map.serialize_entry("options", &self.options)?;
        map.end()
    }
}
