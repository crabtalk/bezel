//! What the agentflow parser builds. Pele's `diagrams/agentflow/db.ts`.

use crate::js::JsValue;
use indexmap::{IndexMap, IndexSet};
use serde::{Serialize, Serializer, ser::SerializeMap};

use super::{
    graph::AgentGraph,
    shapes::{ShapeSink, is_valid_shape, resolve_shape_alias},
    types::{
        AgentClassDef, AgentEdge, AgentNode, AgentSubgraph, Diagnostic, DiagnosticId, DocItem,
        EdgeSemantic, ElementMapping, ElementPosition, FlowText, LabelType, LinkInfo, Loc,
        Metadata, SemanticConnector, SemanticEdge, SemanticModel, SemanticSubgraph, SemanticVertex,
        Severity, StatementType, Vertex, VertexKind,
    },
};
use crate::diagrams::flowchart::syntax::FlowGraph;
use crate::{
    config::{Map, Value},
    detect::Kind,
    error::Error,
    js::{self, Lazy},
    util::url::safe_url,
    yaml::parse_yaml,
};

/// Presentation-only metadata keys, left out of the semantic model.
const PRESENTATION_KEYS: [&str; 11] = [
    "shape",
    "view",
    "icon",
    "img",
    "form",
    "pos",
    "w",
    "h",
    "class",
    "style",
    "labelType",
];
const TOOL_SHAPES: [&str; 5] = [
    "subroutine",
    "subprocess",
    "subproc",
    "framed-rectangle",
    "tool",
];

static RE_BLOCK_SCALAR: Lazy = Lazy::new(r":\s*[>|][\d+-]*\s*$", "");
static RE_TRAILING_COMMA: Lazy = Lazy::new(r",([\t ]*)$", "");
static RE_LEAD: Lazy = Lazy::new(r"^\s+", "g");
static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentflowDbOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_edges: Option<usize>,
    pub inherit_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// Lines of front matter before the text the parser sees, added to every
    /// reported line.
    pub line_offset: usize,
}

pub fn is_tool_definition(vertex: &AgentNode) -> bool {
    vertex
        .kind
        .as_deref()
        .is_some_and(|kind| TOOL_SHAPES.contains(&kind))
}

fn label_type(kind: Option<&Value>) -> LabelType {
    match kind.and_then(Value::as_str) {
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

/// Drops `__proto__`, `constructor` and `prototype` keys at every level, so
/// that a consumer merging metadata into its own objects cannot be made to
/// pollute a prototype.
fn strip_prototype_keys(root: Value) -> Value {
    // The nesting is walked with a stack of its own, so depth is limited by
    // memory and not by the call stack.
    enum Step {
        Visit(Value),
        CloseList(usize),
        CloseMap(Vec<String>),
    }
    let mut steps = vec![Step::Visit(root)];
    let mut done: Vec<Value> = Vec::new();
    while let Some(step) = steps.pop() {
        match step {
            Step::Visit(Value::Array(items)) => {
                steps.push(Step::CloseList(items.len()));
                steps.extend(items.into_iter().rev().map(Step::Visit));
            }
            Step::Visit(Value::Object(map)) => {
                let mut keys = Vec::new();
                let mut values = Vec::new();
                for (key, value) in map.iter() {
                    if matches!(key.as_str(), "__proto__" | "constructor" | "prototype") {
                        continue;
                    }
                    keys.push(key.to_string());
                    values.push(value.clone());
                }
                steps.push(Step::CloseMap(keys));
                steps.extend(values.into_iter().rev().map(Step::Visit));
            }
            Step::Visit(value) => done.push(value),
            Step::CloseList(n) => {
                let items = done.split_off(done.len() - n);
                done.push(Value::Array(items));
            }
            Step::CloseMap(keys) => {
                let values = done.split_off(done.len() - keys.len());
                let mut map = Map::new();
                for (key, value) in keys.into_iter().zip(values) {
                    map.insert(key, value);
                }
                done.push(Value::Object(map));
            }
        }
    }
    done.pop().unwrap_or_default()
}

/// Adds what a metadata block holds to an element's metadata. Mermaid
/// spreads the parsed YAML whatever it is, so a list adds its indexes as keys
/// and a string its characters.
fn merge(into: Option<Metadata>, doc: &Value) -> Metadata {
    let mut out = into.unwrap_or_default();
    match doc {
        Value::Object(map) => {
            for (key, value) in map.iter() {
                out.insert(key.clone(), value.clone());
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                out.insert(i.to_string(), item.clone());
            }
        }
        Value::String(text) => {
            for (i, unit) in text.encode_utf16().enumerate() {
                out.insert(
                    i.to_string(),
                    Value::String(String::from_utf16_lossy(&[unit])),
                );
            }
        }
        _ => {}
    }
    out
}

/// Removes the comma that ends a line of a multi-line metadata body, which
/// block YAML does not allow. Commas inside quoted scalars, flow collections
/// and block scalars are content and stay.
fn strip_line_trailing_commas(body: &str) -> String {
    let mut in_single = false;
    let mut in_double = false;
    let mut flow_depth: usize = 0;
    let mut block_scalar_indent: Option<usize> = None;
    let mut out: Vec<String> = Vec::new();
    for line in body.split('\n') {
        let indent = js::len(line) - js::len(js::trim_start(line));
        if let Some(block) = block_scalar_indent {
            if js::trim(line).is_empty() || indent > block {
                out.push(line.to_string());
                continue;
            }
            block_scalar_indent = None;
        }
        let bytes = line.as_bytes();
        let mut comment_start: Option<usize> = None;
        let mut i = 0;
        while i < bytes.len() {
            let ch = bytes[i];
            if in_double {
                if ch == b'\\' {
                    i += 1;
                } else if ch == b'"' {
                    in_double = false;
                }
            } else if in_single {
                if ch == b'\'' {
                    in_single = false;
                }
            } else if ch == b'"' {
                in_double = true;
            } else if ch == b'\'' {
                in_single = true;
            } else if ch == b'#' && (i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
                comment_start = Some(i);
                break;
            } else if ch == b'[' || ch == b'{' {
                flow_depth += 1;
            } else if ch == b']' || ch == b'}' {
                flow_depth = flow_depth.saturating_sub(1);
            }
            i += 1;
        }
        if in_single || in_double || flow_depth > 0 {
            out.push(line.to_string());
            continue;
        }
        let (code, comment) = match comment_start {
            Some(at) => line.split_at(at),
            None => (line, ""),
        };
        if RE_BLOCK_SCALAR.test(code) {
            block_scalar_indent = Some(indent);
            out.push(line.to_string());
            continue;
        }
        out.push(RE_TRAILING_COMMA.replace(code, |caps| caps.str(1).to_string()) + comment);
    }
    out.join("\n")
}

fn edge_semantic(kind: &str, stroke: &str) -> Option<EdgeSemantic> {
    match (kind, stroke) {
        ("arrow_point", "normal") => Some(EdgeSemantic::Sequence),
        ("arrow_cross", "normal") => Some(EdgeSemantic::Failure),
        ("arrow_open", "dotted") => Some(EdgeSemantic::Reference),
        _ => None,
    }
}

fn destruct_end_link(raw: &str) -> LinkInfo {
    let text = js::trim(raw);
    let mut line = text.get(..text.len().saturating_sub(1)).unwrap_or("");
    let mut kind = "arrow_open";
    match text.as_bytes().last() {
        Some(b'x') => kind = "arrow_cross",
        Some(b'>') => kind = "arrow_point",
        Some(b'-' | b'.') => line = text,
        _ => {}
    }
    let mut stroke = "normal";
    let mut length = js::len(line) as i32 - 1;
    let dots = line.matches('.').count() as i32;
    if dots > 0 {
        stroke = "dotted";
        length = dots;
    }
    LinkInfo {
        kind: kind.into(),
        stroke: stroke.into(),
        length: Some(length),
        ..LinkInfo::default()
    }
}

fn semantic_metadata(metadata: Option<&Metadata>) -> Option<Metadata> {
    let metadata = metadata?;
    let mut out = Map::new();
    for (key, value) in metadata.iter() {
        if PRESENTATION_KEYS.contains(&key.as_str()) {
            continue;
        }
        out.insert(key.clone(), value.clone());
    }
    (!out.is_empty()).then_some(out)
}

/// The agentflow model. Serializes as Pele's `AgentflowDb` does through
/// `JSON.stringify`, its bookkeeping included.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentflowDb {
    pub direction: Option<String>,
    pub nodes: IndexMap<String, AgentNode>,
    pub edges: Vec<AgentEdge>,
    pub subgraphs: Vec<AgentSubgraph>,
    /// The connectors' ids; each is the node of that id in `nodes`.
    pub connectors: IndexSet<String>,
    pub classes: IndexMap<String, AgentClassDef>,
    pub tooltips: IndexMap<String, String>,
    pub default_edge_style: Option<Vec<String>>,
    pub default_interpolate: Option<String>,
    /// Where each statement sits in the source, in the order the statements
    /// were read.
    pub mappings: Vec<ElementMapping>,
    pub diagnostics: Vec<Diagnostic>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub line_offset: usize,
    /// What is drawn, built once the source is read.
    pub graph: AgentGraph,

    /// Subgraphs by id, as indices into `subgraphs`.
    subgraph_lookup: IndexMap<String, usize>,
    global_nodes: IndexSet<String>,
    members: IndexSet<String>,
    /// Edges by id, as indices into `edges`.
    edge_by_id: IndexMap<String, usize>,
    pair_count: IndexMap<String, usize>,
    sub_count: usize,
    /// Indices into `mappings`.
    bare_mappings: IndexSet<usize>,
    /// Indices into `mappings`.
    first_mapping: IndexMap<String, usize>,
    /// Indices into `diagnostics`.
    reported: IndexMap<String, Vec<usize>>,
    options: AgentflowDbOptions,
}

impl AgentflowDb {
    pub fn new(options: AgentflowDbOptions) -> Self {
        Self {
            direction: None,
            nodes: IndexMap::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
            connectors: IndexSet::new(),
            classes: IndexMap::new(),
            tooltips: IndexMap::new(),
            default_edge_style: None,
            default_interpolate: None,
            mappings: Vec::new(),
            diagnostics: Vec::new(),
            title: None,
            acc_title: None,
            acc_descr: None,
            line_offset: options.line_offset,
            graph: AgentGraph::default(),
            subgraph_lookup: IndexMap::new(),
            global_nodes: IndexSet::new(),
            members: IndexSet::new(),
            edge_by_id: IndexMap::new(),
            pair_count: IndexMap::new(),
            sub_count: 0,
            bare_mappings: IndexSet::new(),
            first_mapping: IndexMap::new(),
            reported: IndexMap::new(),
            options,
        }
    }

    fn place(&self, loc: Option<Loc>) -> (usize, usize) {
        match loc {
            Some(loc) => (loc.first_line + self.line_offset, loc.first_column + 1),
            None => (0, 0),
        }
    }

    fn metadata(&self, id: &str, source: &str, loc: Option<Loc>) -> Result<Value, Error> {
        let multiline = source.contains('\n');
        let first = if multiline {
            parse_yaml(&format!("{source}\n"))
        } else {
            parse_yaml(&format!("{{\n{source}\n}}"))
        };
        let parsed = match first {
            Ok(parsed) => parsed,
            Err(error) => {
                // Block YAML rejects the comma-separated style of the
                // single-line form, so try once more without the commas.
                let stripped = if multiline {
                    strip_line_trailing_commas(source)
                } else {
                    source.to_string()
                };
                let retried = if stripped != source {
                    parse_yaml(&format!("{stripped}\n")).ok()
                } else {
                    None
                };
                match retried {
                    Some(retried) => retried,
                    None => {
                        let at = match loc {
                            Some(loc) => format!(
                                " ({}:{})",
                                loc.first_line + self.line_offset,
                                loc.first_column + 1
                            ),
                            None => String::new(),
                        };
                        let (line, column) = self.place(loc);
                        let mut error = Error::syntax(format!(
                            "Metadata of \"{id}\" is not valid YAML. {}{at}",
                            error.0
                        ))
                        .of(Kind::Agentflow);
                        error.line = line;
                        error.column = column;
                        return Err(error);
                    }
                }
            }
        };
        let parsed = if parsed == Value::Null {
            Value::Object(Map::new())
        } else {
            parsed
        };
        Ok(strip_prototype_keys(parsed))
    }

    fn shape_error(&self, message: String, loc: Option<Loc>) -> Error {
        let (line, column) = self.place(loc);
        let mut error = Error::semantic(message).of(Kind::Agentflow);
        error.line = line;
        error.column = column;
        error
    }

    /// [`FlowGraph::add_vertex`], with `metadata_loc` where
    /// `vertex.metadata` was written.
    pub fn add_vertex_at(
        &mut self,
        id: &str,
        vertex: Vertex<'_>,
        metadata_loc: Option<Loc>,
    ) -> Result<(), Error> {
        if id.is_empty() || js::trim(id).is_empty() {
            return Ok(());
        }

        let mut doc: Option<Value> = None;
        let mut authored_shape: Option<String> = None;
        if let Some(metadata) = vertex.metadata {
            let mut parsed = self.metadata(id, metadata, metadata_loc)?;
            if let Value::Object(fields) = &mut parsed
                && let Some(Value::String(shape)) = fields.get("shape")
            {
                let shape = shape.clone();
                let resolved = resolve_shape_alias(Some(&shape))
                    .unwrap_or_default()
                    .to_string();
                authored_shape = Some(shape);
                fields.insert("shape".into(), Value::String(resolved));
            }
            doc = Some(parsed);
        }
        let doc = doc.filter(Value::truthy);

        if let Some(&at) = self.subgraph_lookup.get(id)
            && let Some(doc) = &doc
        {
            let subgraph = &mut self.subgraphs[at];
            subgraph.metadata = Some(merge(subgraph.metadata.take(), doc));
            return Ok(());
        }

        if self.connectors.contains(id)
            && let Some(doc) = &doc
            && let Some(connector) = self.nodes.get_mut(id)
        {
            connector.metadata = Some(merge(connector.metadata.take(), doc));
            return Ok(());
        }

        // Mermaid reserves this id and creates no node for it.
        if id == "connectors" {
            return Ok(());
        }

        if let Some(&at) = self.edge_by_id.get(id) {
            let edge = &mut self.edges[at];
            if let Some(Value::Object(fields)) = &doc {
                if let Some(value) = fields.get("animate") {
                    edge.animate = Some(value.clone());
                }
                if let Some(value) = fields.get("animation") {
                    edge.animation = Some(value.clone());
                }
                if let Some(value) = fields.get("curve") {
                    edge.interpolate = Some(value.clone());
                }
            }
            if let Some(doc) = &doc {
                edge.metadata = Some(merge(edge.metadata.take(), doc));
            }
            return Ok(());
        }

        let node = self
            .nodes
            .entry(id.to_string())
            .or_insert_with(|| AgentNode {
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
                metadata: None,
                is_connector: None,
            });

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

        let Some(doc) = doc else {
            return Ok(());
        };
        node.metadata = Some(merge(node.metadata.take(), &doc));
        let Value::Object(fields) = doc else {
            return Ok(());
        };

        if let Some(shape) = fields.get("shape").filter(|v| v.truthy()) {
            let authored = match &authored_shape {
                Some(authored) => authored.clone(),
                None => {
                    let message = format!("No such shape: {}.", shape.to_json());
                    return Err(self.shape_error(message, metadata_loc));
                }
            };
            if authored != authored.to_lowercase() || authored.contains('_') {
                let message =
                    format!("No such shape: {authored}. Shape names should be lowercase.");
                return Err(self.shape_error(message, metadata_loc));
            }
            let shape = shape.to_js_string();
            if !is_valid_shape(&shape) {
                return Err(self.shape_error(format!("No such shape: {shape}."), metadata_loc));
            }
            if let Some(vertex) = self.nodes.get_mut(id) {
                vertex.kind = Some(shape);
            }
        }
        if let Some(label) = fields.get("label").filter(|v| v.truthy())
            && let Some(vertex) = self.nodes.get_mut(id)
        {
            vertex.text = Some(label.to_js_string());
            vertex.label_type = label_type(fields.get("labelType"));
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
        let mut edge = AgentEdge {
            start: start.to_string(),
            end: end.to_string(),
            kind: None,
            text: String::new(),
            label_type: LabelType::Text,
            stroke: None,
            edge_semantic: None,
            length: None,
            style: None,
            classes: Vec::new(),
            id: None,
            is_user_defined_id: false,
            interpolate: self.default_interpolate.clone().map(Value::String),
            animate: None,
            animation: None,
            metadata: None,
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
            edge.edge_semantic = edge_semantic(&info.kind, &info.stroke);
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
            .of(Kind::Agentflow));
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
        let index = js::number(pos);
        if index >= 0.0 && index.fract() == 0.0 && (index as usize) < self.edges.len() {
            return Ok(index as usize);
        }
        Err(Error::semantic(format!(
            "The index {pos} for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and {}. (Help: Ensure that the index is within the range of existing edges.)",
            self.edges.len() as i64 - 1
        ))
        .of(Kind::Agentflow))
    }

    pub fn add_sub_graph(
        &mut self,
        id_text: Option<&FlowText>,
        list: &[DocItem],
        title_text: Option<&FlowText>,
    ) -> String {
        let id: Option<String> = id_text.map(|t| js::trim(&t.text).to_string());
        let title = title_text.map_or("", |t| t.text.as_str());

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
            dir = self
                .direction
                .clone()
                .or_else(|| self.options.direction.clone());
        }

        let id = id.unwrap_or_else(|| format!("subGraph{}", self.sub_count));
        self.sub_count += 1;

        // A flow never contains itself, a globally scoped node, or a node
        // another flow already holds.
        let nodes: Vec<String> = node_list
            .into_iter()
            .filter(|node| {
                *node != id && !self.global_nodes.contains(node) && !self.members.contains(node)
            })
            .collect();
        for node in &nodes {
            self.members.insert(node.clone());
        }

        if let Some(&at) = self.subgraph_lookup.get(&id) {
            // A repeated id adds its members to the first flow; its title
            // and direction win.
            let existing = &mut self.subgraphs[at];
            existing.nodes.extend(nodes);
            if !js::trim(title).is_empty() {
                existing.title = js::trim(title).to_string();
            }
            if let Some(dir) = dir.filter(|dir| !dir.is_empty()) {
                existing.dir = Some(dir);
            }
            return id;
        }
        self.subgraphs.push(AgentSubgraph {
            id: id.clone(),
            nodes,
            title: js::trim(title).to_string(),
            classes: Vec::new(),
            dir,
            label_type: title_text.map_or(LabelType::Markdown, |t| t.kind),
            kind: "flow",
            metadata: None,
        });
        self.subgraph_lookup
            .insert(id.clone(), self.subgraphs.len() - 1);
        id
    }

    /// Every node named inside a `global` block stays out of the flows that
    /// mention it.
    pub fn add_global(&mut self, list: &[DocItem]) {
        let mut release = false;
        let mut add = |item: &str, db: &mut Self| {
            let id = js::trim(item);
            if id.is_empty() {
                return;
            }
            db.global_nodes.insert(id.to_string());
            if db.members.shift_remove(id) {
                release = true;
            }
        };
        for entry in list {
            match entry {
                DocItem::Str(entry) => add(entry, self),
                DocItem::List(items) => {
                    for item in items {
                        add(item, self);
                    }
                }
                DocItem::Dir(_) => {}
            }
        }
        if !release {
            return;
        }
        for subgraph in &mut self.subgraphs {
            subgraph
                .nodes
                .retain(|node| !self.global_nodes.contains(node));
        }
    }

    pub fn add_connector(
        &mut self,
        text_obj: Option<&FlowText>,
        title_obj: Option<&FlowText>,
    ) -> String {
        let id = text_obj.map_or("", |t| js::trim(&t.text)).to_string();
        if id.is_empty() {
            return String::new();
        }
        let title = title_obj.map_or("", |t| js::trim(&t.text));
        if !self.connectors.contains(&id) {
            let created = AgentNode {
                id: id.clone(),
                text: Some(if title.is_empty() {
                    id.clone()
                } else {
                    title.to_string()
                }),
                label_type: LabelType::Text,
                kind: None,
                styles: Vec::new(),
                classes: Vec::new(),
                dir: None,
                props: None,
                link: None,
                link_target: None,
                metadata: None,
                is_connector: Some(true),
            };
            self.connectors.insert(id.clone());
            self.nodes.insert(id.clone(), created);
        } else if !title.is_empty()
            && let Some(connector) = self.nodes.get_mut(&id)
        {
            connector.text = Some(title.to_string());
        }
        id
    }

    pub fn subgraph(&self, id: &str) -> Option<&AgentSubgraph> {
        self.subgraph_lookup.get(id).map(|&at| &self.subgraphs[at])
    }

    pub fn vertex_kind(&self, vertex: &AgentNode) -> VertexKind {
        if is_tool_definition(vertex) {
            return VertexKind::Tool;
        }
        match resolve_shape_alias(vertex.kind.as_deref()) {
            Some("hexagon" | "hex") => VertexKind::Action,
            Some("lean-right" | "lean_right") => VertexKind::Input,
            Some("lin-doc" | "lined-document") => VertexKind::Refdoc,
            Some("diamond") => VertexKind::Decision,
            _ => VertexKind::Task,
        }
    }

    pub fn semantic_model(&self) -> SemanticModel {
        let mut vertices = Vec::new();
        let mut connectors = Vec::new();
        for (id, v) in &self.nodes {
            if self.subgraph_lookup.contains_key(id) {
                continue;
            }
            let metadata = semantic_metadata(v.metadata.as_ref());
            if v.is_connector == Some(true) {
                connectors.push(SemanticConnector {
                    id: id.clone(),
                    title: v.text.clone().filter(|text| text != id),
                    metadata,
                });
                continue;
            }
            vertices.push(SemanticVertex {
                id: id.clone(),
                label: v.text.clone(),
                shape: resolve_shape_alias(v.kind.as_deref()).map(str::to_string),
                vertex_kind: Some(self.vertex_kind(v)),
                metadata,
            });
        }

        let edges = self
            .edges
            .iter()
            .map(|e| SemanticEdge {
                start: e.start.clone(),
                end: e.end.clone(),
                id: e.id.clone(),
                label: (!e.text.is_empty()).then(|| e.text.clone()),
                kind: e.kind.clone(),
                stroke: e.stroke.clone(),
                edge_semantic: e.edge_semantic,
                length: e.length,
                metadata: e.metadata.clone().filter(|m| !m.is_empty()),
            })
            .collect();

        let sub_graphs = self
            .subgraphs
            .iter()
            .map(|sg| SemanticSubgraph {
                id: sg.id.clone(),
                kind: Some(sg.kind.to_string()),
                title: Some(sg.title.clone()),
                nodes: sg.nodes.clone(),
                metadata: semantic_metadata(sg.metadata.as_ref()),
                direction: sg.dir.clone(),
            })
            .collect();

        SemanticModel {
            direction: self.direction.clone(),
            vertices,
            edges,
            sub_graphs,
            connectors,
            diagnostics: self.diagnostics.clone(),
        }
    }

    fn position(&self, loc: Option<Loc>) -> ElementPosition {
        let start_line = loc.map_or(0, |l| l.first_line);
        let start_column = loc.map_or(0, |l| l.first_column);
        ElementPosition {
            start_line: start_line + self.line_offset,
            start_column,
            end_line: loc.map_or(start_line, |l| l.last_line) + self.line_offset,
            end_column: loc.map_or(start_column, |l| l.last_column),
            // Mermaid builds its parser without ranges, so these are always
            // zero.
            start_index: 0,
            end_index: 0,
        }
    }

    fn push_mapping(&mut self, id: &str, kind: StatementType, position: ElementPosition) -> usize {
        self.mappings.push(ElementMapping {
            id: id.to_string(),
            kind,
            position,
        });
        let at = self.mappings.len() - 1;
        if !self.first_mapping.contains_key(id) {
            self.first_mapping.insert(id.to_string(), at);
        }
        at
    }

    /// `bare` when the vertex names no shape.
    pub fn add_vertex_mapping(&mut self, id: &str, bare: bool, loc: Option<Loc>) {
        if id.is_empty() {
            return;
        }
        let position = self.position(loc);
        let at = self.push_mapping(id, StatementType::Vertex, position);
        // A bare reference becomes an attachment if a metadata block follows
        // it.
        if bare {
            self.bare_mappings.insert(at);
        }
    }

    /// Widens a node's span over the metadata block that follows it.
    pub fn extend_vertex_mapping(&mut self, id: &str, loc: Option<Loc>) {
        if id.is_empty() || loc.is_none() {
            return;
        }
        let end = self.position(loc);
        for i in (0..self.mappings.len()).rev() {
            let m = &self.mappings[i];
            if m.kind != StatementType::Vertex || m.id != id {
                continue;
            }
            if self.bare_mappings.shift_remove(&i) {
                self.mappings[i].kind = StatementType::Attachment;
            }
            let p = &mut self.mappings[i].position;
            if end.end_line > p.end_line
                || (end.end_line == p.end_line && end.end_column > p.end_column)
            {
                p.end_line = end.end_line;
                p.end_column = end.end_column;
            }
            return;
        }
        self.push_mapping(id, StatementType::Attachment, end);
    }

    pub fn add_edge_mapping(&mut self, to_nodes: &[String], loc: Option<Loc>) {
        let ids: Vec<&str> = to_nodes
            .iter()
            .map(String::as_str)
            .filter(|n| !n.is_empty())
            .collect();
        let id = if ids.is_empty() {
            "edge".to_string()
        } else {
            ids.join(">")
        };
        let position = self.position(loc);
        self.push_mapping(&id, StatementType::Edge, position);
    }

    fn block_mapping(
        &mut self,
        id: &str,
        kind: StatementType,
        start_loc: Option<Loc>,
        end_loc: Option<Loc>,
    ) {
        if id.is_empty() {
            return;
        }
        let start = self.position(start_loc);
        let end = if end_loc.is_some() {
            self.position(end_loc)
        } else {
            start
        };
        self.push_mapping(
            id,
            kind,
            ElementPosition {
                start_line: start.start_line,
                start_column: start.start_column,
                end_line: end.end_line,
                end_column: end.end_column,
                start_index: 0,
                end_index: 0,
            },
        );
    }

    pub fn add_subgraph_mapping(
        &mut self,
        id: Option<&FlowText>,
        start_loc: Option<Loc>,
        end_loc: Option<Loc>,
    ) {
        let text = id.map_or("", |t| t.text.as_str()).to_string();
        self.block_mapping(&text, StatementType::Subgraph, start_loc, end_loc);
    }

    pub fn add_connector_mapping(
        &mut self,
        text_obj: Option<&FlowText>,
        start_loc: Option<Loc>,
        end_loc: Option<Loc>,
    ) {
        let id = text_obj.map_or("", |t| js::trim(&t.text)).to_string();
        self.block_mapping(&id, StatementType::Connector, start_loc, end_loc);
    }

    fn emit(
        &mut self,
        id: DiagnosticId,
        severity: Severity,
        message: String,
        node_id: Option<&str>,
        edge_id: Option<&str>,
    ) {
        let anchor = node_id.or(edge_id);
        let mut diagnostic = Diagnostic {
            id,
            severity,
            message,
            node_id: None,
            edge_id: None,
            position: None,
        };
        if let Some(node_id) = node_id.filter(|id| !id.is_empty()) {
            diagnostic.node_id = Some(node_id.to_string());
        } else if let Some(edge_id) = edge_id.filter(|id| !id.is_empty()) {
            diagnostic.edge_id = Some(edge_id.to_string());
        }
        if let Some(&at) = anchor
            .filter(|anchor| !anchor.is_empty())
            .and_then(|anchor| self.first_mapping.get(anchor))
        {
            diagnostic.position = Some(self.mappings[at].position);
        }

        // The graph may be built more than once; each finding is recorded
        // once.
        let key = anchor.unwrap_or("").to_string();
        let same = self.reported.entry(key).or_default();
        for &d in same.iter() {
            let d = &self.diagnostics[d];
            if d.id == id
                && d.severity == severity
                && d.message == diagnostic.message
                && d.node_id == diagnostic.node_id
                && d.edge_id == diagnostic.edge_id
            {
                return;
            }
        }
        same.push(self.diagnostics.len());
        self.diagnostics.push(diagnostic);
    }

    pub fn emit_warning(&mut self, id: DiagnosticId, message: String, node_id: &str) {
        self.emit(id, Severity::Warning, message, Some(node_id), None);
    }

    pub fn emit_error(&mut self, id: DiagnosticId, message: String, node_id: &str) {
        self.emit(id, Severity::Error, message, Some(node_id), None);
    }
}

impl ShapeSink for AgentflowDb {
    fn shape_removed(&mut self, message: String, node_id: &str) {
        self.emit_error(DiagnosticId::ShapeRemoved, message, node_id);
    }

    fn shape_unsupported(&mut self, message: String, node_id: &str) {
        self.emit_warning(DiagnosticId::ShapeUnsupported, message, node_id);
    }
}

impl FlowGraph for AgentflowDb {
    fn add_vertex(&mut self, id: &str, vertex: Vertex<'_>) -> Result<(), Error> {
        self.add_vertex_at(id, vertex, None)
    }

    fn destruct_link(&self, end: &str, start: Option<&str>) -> LinkInfo {
        let info = destruct_end_link(end);
        if let Some(start) = start.filter(|start| !start.is_empty()) {
            // The first half of a split arrow only says whether the line is
            // dotted.
            let stroke = if start.contains('.') {
                "dotted"
            } else {
                "normal"
            };
            if stroke != info.stroke {
                return LinkInfo {
                    kind: "INVALID".into(),
                    stroke: "INVALID".into(),
                    ..LinkInfo::default()
                };
            }
        }
        info
    }

    fn update_link_interpolate(
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
    fn update_link(&mut self, positions: &[String], mut style: Vec<String>) -> Result<(), Error> {
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

    fn add_class(&mut self, ids: &str, styles: &[String]) {
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
                .or_insert_with(|| AgentClassDef {
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

    fn set_direction(&mut self, dir: &str) {
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

    fn set_class(&mut self, ids: &str, class_name: &str) {
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

    fn set_tooltip(&mut self, ids: &str, tooltip: &str) {
        for id in ids.split(',') {
            self.tooltips.insert(id.to_string(), tooltip.to_string());
        }
    }

    fn set_link(&mut self, ids: &str, link: &str, target: Option<&str>) {
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
    fn set_click_event(&mut self, ids: &str) {
        self.set_class(ids, "clickable");
    }

    fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(RE_LEAD.replace(text, |_| String::new()));
    }

    fn set_acc_description(&mut self, text: &str) {
        self.acc_descr = Some(RE_INDENT.replace(text, |_| "\n".into()));
    }
}

impl Serialize for AgentflowDb {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("type", "agentflow")?;
        if let Some(direction) = &self.direction {
            map.serialize_entry("direction", direction)?;
        }
        map.serialize_entry("nodes", &self.nodes)?;
        map.serialize_entry("edges", &self.edges)?;
        map.serialize_entry("subgraphs", &self.subgraphs)?;
        let connectors: IndexMap<&str, &AgentNode> = self
            .connectors
            .iter()
            .filter_map(|id| self.nodes.get(id).map(|node| (id.as_str(), node)))
            .collect();
        map.serialize_entry("connectors", &connectors)?;
        map.serialize_entry("classes", &self.classes)?;
        map.serialize_entry("tooltips", &self.tooltips)?;
        if let Some(style) = &self.default_edge_style {
            map.serialize_entry("defaultEdgeStyle", style)?;
        }
        if let Some(interpolate) = &self.default_interpolate {
            map.serialize_entry("defaultInterpolate", interpolate)?;
        }
        map.serialize_entry("mappings", &self.mappings)?;
        map.serialize_entry("diagnostics", &self.diagnostics)?;
        if let Some(title) = &self.title {
            map.serialize_entry("title", title)?;
        }
        if let Some(acc_title) = &self.acc_title {
            map.serialize_entry("accTitle", acc_title)?;
        }
        if let Some(acc_descr) = &self.acc_descr {
            map.serialize_entry("accDescr", acc_descr)?;
        }
        map.serialize_entry("lineOffset", &self.line_offset)?;
        let lookup: IndexMap<&str, &AgentSubgraph> = self
            .subgraph_lookup
            .iter()
            .map(|(id, &at)| (id.as_str(), &self.subgraphs[at]))
            .collect();
        map.serialize_entry("subgraphLookup", &lookup)?;
        map.serialize_entry("globalNodes", &self.global_nodes)?;
        map.serialize_entry("members", &self.members)?;
        let by_id: IndexMap<&str, &AgentEdge> = self
            .edge_by_id
            .iter()
            .map(|(id, &at)| (id.as_str(), &self.edges[at]))
            .collect();
        map.serialize_entry("edgeById", &by_id)?;
        map.serialize_entry("pairCount", &self.pair_count)?;
        map.serialize_entry("subCount", &self.sub_count)?;
        let bare: Vec<&ElementMapping> = self
            .bare_mappings
            .iter()
            .map(|&at| &self.mappings[at])
            .collect();
        map.serialize_entry("bareMappings", &bare)?;
        let first: IndexMap<&str, &ElementMapping> = self
            .first_mapping
            .iter()
            .map(|(id, &at)| (id.as_str(), &self.mappings[at]))
            .collect();
        map.serialize_entry("firstMapping", &first)?;
        let reported: IndexMap<&str, Vec<&Diagnostic>> = self
            .reported
            .iter()
            .map(|(key, all)| {
                (
                    key.as_str(),
                    all.iter().map(|&at| &self.diagnostics[at]).collect(),
                )
            })
            .collect();
        map.serialize_entry("reported", &reported)?;
        map.serialize_entry("options", &self.options)?;
        map.end()
    }
}
