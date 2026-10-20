//! What the agentflow parser builds. Pele's `diagrams/agentflow/db.ts`.

use indexmap::{IndexMap, IndexSet};
use serde::{Serialize, Serializer, ser::SerializeMap};

use super::{
    Agentflow,
    graph::AgentGraph,
    shapes::{ShapeSink, is_valid_shape, resolve_shape_alias},
    types::{
        AgentNode, ArrowType, Diagnostic, DiagnosticId, DocItem, ElementMapping, ElementPosition,
        FlowText, LinkInfo, Loc, Metadata, Node, NodeExt, SemanticConnector, SemanticEdge,
        SemanticModel, SemanticSubgraph, SemanticVertex, Severity, StatementType, Stroke, Vertex,
        VertexKind,
    },
};
use crate::{
    config::{Map, Value},
    detect::Kind,
    diagrams::flowchart::store::{Element, FlowStore, Repeat, StoreOptions},
    error::Error,
    js::{self, JsValue, Lazy},
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

impl AgentNode {
    pub fn is_tool_definition(&self) -> bool {
        self.kind
            .as_deref()
            .is_some_and(|kind| TOOL_SHAPES.contains(&kind))
    }

    pub fn vertex_kind(&self) -> VertexKind {
        if self.is_tool_definition() {
            return VertexKind::Tool;
        }
        match resolve_shape_alias(self.kind.as_deref()) {
            Some("hexagon" | "hex") => VertexKind::Action,
            Some("lean-right" | "lean_right") => VertexKind::Input,
            Some("lin-doc" | "lined-document") => VertexKind::Refdoc,
            Some("diamond") => VertexKind::Decision,
            _ => VertexKind::Task,
        }
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

fn destruct_end_link(raw: &str) -> LinkInfo {
    let text = js::trim(raw);
    let mut line = text.get(..text.len().saturating_sub(1)).unwrap_or("");
    let mut kind = ArrowType::Open;
    match text.as_bytes().last() {
        Some(b'x') => kind = ArrowType::Cross,
        Some(b'>') => kind = ArrowType::Point,
        Some(b'-' | b'.') => line = text,
        _ => {}
    }
    let mut stroke = Stroke::Normal;
    let mut length = line.len() as i32 - 1;
    let dots = line.matches('.').count() as i32;
    if dots > 0 {
        stroke = Stroke::Dotted;
        length = dots;
    }
    LinkInfo {
        kind,
        stroke,
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

/// The link a `LINK` token and the `START_LINK` before it, if any, draw.
pub fn destruct_link(end: &str, start: Option<&str>) -> LinkInfo {
    let info = destruct_end_link(end);
    if let Some(start) = start.filter(|start| !start.is_empty()) {
        // The first half of a split arrow only says whether the line is
        // dotted.
        let stroke = if start.contains('.') {
            Stroke::Dotted
        } else {
            Stroke::Normal
        };
        if stroke != info.stroke {
            return LinkInfo::invalid();
        }
    }
    info
}

fn located(mut error: Error, (line, column): (usize, usize)) -> Error {
    error.line = line;
    error.column = column;
    error
}

/// The agentflow model. Serializes as Pele's `AgentflowDb` does through
/// `JSON.stringify`, its bookkeeping included.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentflowDb {
    pub store: FlowStore<Agentflow>,
    /// The connectors' ids; each is the node of that id in `store.nodes`.
    pub connectors: IndexSet<String>,
    /// Where each statement sits in the source, in the order the statements
    /// were read.
    pub mappings: Vec<ElementMapping>,
    pub diagnostics: Vec<Diagnostic>,
    /// What is drawn, built once the source is read.
    pub graph: AgentGraph,

    global_nodes: IndexSet<String>,
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
            store: FlowStore::new(StoreOptions {
                inherit_dir: options.inherit_dir,
                max_edges: options.max_edges,
            }),
            connectors: IndexSet::new(),
            mappings: Vec::new(),
            diagnostics: Vec::new(),
            graph: AgentGraph::default(),
            global_nodes: IndexSet::new(),
            bare_mappings: IndexSet::new(),
            first_mapping: IndexMap::new(),
            reported: IndexMap::new(),
            options,
        }
    }

    /// The line and column of `loc`'s start, as errors report them.
    fn place(&self, loc: Loc) -> (usize, usize) {
        (
            loc.first_line + self.options.line_offset,
            loc.first_column + 1,
        )
    }

    fn metadata(&self, id: &str, source: &str, loc: Loc) -> Result<Value, Error> {
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
                        let (line, column) = self.place(loc);
                        let error = Error::syntax(format!(
                            "Metadata of \"{id}\" is not valid YAML. {} ({line}:{column})",
                            error.0
                        ))
                        .of(Kind::Agentflow);
                        return Err(located(error, (line, column)));
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

    /// Applies a `@{ … }` block, written at `loc`, to what `id` names: a
    /// flow's, connector's, edge's or node's metadata, and a node's shape and
    /// label or an edge's animation and curve.
    pub fn add_metadata(&mut self, id: &str, source: &str, loc: Loc) -> Result<(), Error> {
        if js::trim(id).is_empty() {
            return Ok(());
        }

        let mut parsed = self.metadata(id, source, loc)?;
        let mut authored_shape: Option<String> = None;
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
        let Some(doc) = Some(parsed).filter(Value::truthy) else {
            self.store.add_vertex(id, Vertex::default());
            return Ok(());
        };

        if let Some(subgraph) = self.store.subgraph_mut(id) {
            subgraph.metadata = Some(merge(subgraph.metadata.take(), &doc));
            return Ok(());
        }
        if self.connectors.contains(id)
            && let Some(connector) = self.store.nodes.get_mut(id)
        {
            connector.ext.metadata = Some(merge(connector.ext.metadata.take(), &doc));
            return Ok(());
        }

        let at = self.place(loc);
        let node = match self.store.add_vertex(id, Vertex::default()) {
            Some(Element::Node(node)) => node,
            Some(Element::Edge(edge)) => {
                if let Value::Object(fields) = &doc {
                    edge.apply_animation(fields);
                }
                edge.ext.metadata = Some(merge(edge.ext.metadata.take(), &doc));
                return Ok(());
            }
            None => return Ok(()),
        };
        node.ext.metadata = Some(merge(node.ext.metadata.take(), &doc));
        let Value::Object(fields) = doc else {
            return Ok(());
        };

        if let Some(shape) = fields.get("shape").filter(|v| v.truthy()) {
            let shape_error = |message| located(Error::semantic(message).of(Kind::Agentflow), at);
            let Some(authored) = authored_shape else {
                return Err(shape_error(format!("No such shape: {}.", shape.to_json())));
            };
            if authored != authored.to_lowercase() || authored.contains('_') {
                return Err(shape_error(format!(
                    "No such shape: {authored}. Shape names should be lowercase."
                )));
            }
            let shape = shape.to_js_string();
            if !is_valid_shape(&shape) {
                return Err(shape_error(format!("No such shape: {shape}.")));
            }
            node.kind = Some(shape);
        }
        node.apply_label(&fields);
        Ok(())
    }

    pub fn add_sub_graph(
        &mut self,
        id_text: Option<&FlowText>,
        list: &[DocItem],
        title_text: Option<&FlowText>,
    ) -> String {
        let id = id_text.map(|t| js::trim(&t.text).to_string());
        let global = &self.global_nodes;
        let (id, repeat) = self
            .store
            .add_sub_graph(id, list, title_text, |node| !global.contains(node));
        // A repeated id adds its members to the first flow; its title and
        // direction win.
        if let Some(Repeat { subgraph, dir }) = repeat {
            let title = js::trim(title_text.map_or("", |t| t.text.as_str()));
            if !title.is_empty() {
                subgraph.title = title.to_string();
            }
            if dir.is_some() {
                subgraph.dir = dir;
            }
        }
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
            if db.store.members.shift_remove(id) {
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
        for subgraph in &mut self.store.subgraphs {
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
            let created = Node {
                text: Some(if title.is_empty() {
                    id.clone()
                } else {
                    title.to_string()
                }),
                ext: NodeExt {
                    metadata: None,
                    is_connector: Some(true),
                },
                ..Node::new(id.clone())
            };
            self.connectors.insert(id.clone());
            self.store.nodes.insert(id.clone(), created);
        } else if !title.is_empty()
            && let Some(connector) = self.store.nodes.get_mut(&id)
        {
            connector.text = Some(title.to_string());
        }
        id
    }

    pub fn semantic_model(&self) -> SemanticModel {
        let mut vertices = Vec::new();
        let mut connectors = Vec::new();
        for (id, v) in &self.store.nodes {
            if self.store.subgraph(id).is_some() {
                continue;
            }
            let metadata = semantic_metadata(v.ext.metadata.as_ref());
            if v.ext.is_connector == Some(true) {
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
                vertex_kind: Some(v.vertex_kind()),
                metadata,
            });
        }

        let edges = self
            .store
            .edges
            .iter()
            .map(|e| SemanticEdge {
                start: e.start.clone(),
                end: e.end.clone(),
                id: e.id.clone(),
                label: (!e.text.is_empty()).then(|| e.text.clone()),
                kind: e.kind,
                stroke: e.stroke,
                edge_semantic: e.line.edge_semantic,
                length: e.length,
                metadata: e.ext.metadata.clone().filter(|m| !m.is_empty()),
            })
            .collect();

        let sub_graphs = self
            .store
            .subgraphs
            .iter()
            .map(|sg| SemanticSubgraph {
                id: sg.id.clone(),
                kind: Some(sg.ext.kind.to_string()),
                title: Some(sg.title.clone()),
                nodes: sg.nodes.clone(),
                metadata: semantic_metadata(sg.metadata.as_ref()),
                direction: sg.dir,
            })
            .collect();

        SemanticModel {
            direction: self.store.direction,
            vertices,
            edges,
            sub_graphs,
            connectors,
            diagnostics: self.diagnostics.clone(),
        }
    }

    fn position(&self, loc: Loc) -> ElementPosition {
        ElementPosition {
            start_line: loc.first_line + self.options.line_offset,
            start_column: loc.first_column,
            end_line: loc.last_line + self.options.line_offset,
            end_column: loc.last_column,
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
    pub fn add_vertex_mapping(&mut self, id: &str, bare: bool, loc: Loc) {
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
    pub fn extend_vertex_mapping(&mut self, id: &str, loc: Loc) {
        if id.is_empty() {
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

    pub fn add_edge_mapping(&mut self, to_nodes: &[String], loc: Loc) {
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

    fn block_mapping(&mut self, id: &str, kind: StatementType, start_loc: Loc, end_loc: Loc) {
        if id.is_empty() {
            return;
        }
        let start = self.position(start_loc);
        let end = self.position(end_loc);
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

    pub fn add_subgraph_mapping(&mut self, id: Option<&FlowText>, start_loc: Loc, end_loc: Loc) {
        let text = id.map_or("", |t| t.text.as_str()).to_string();
        self.block_mapping(&text, StatementType::Subgraph, start_loc, end_loc);
    }

    pub fn add_connector_mapping(
        &mut self,
        text_obj: Option<&FlowText>,
        start_loc: Loc,
        end_loc: Loc,
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

impl Serialize for AgentflowDb {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let store = &self.store;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("type", "agentflow")?;
        if let Some(direction) = &store.direction {
            map.serialize_entry("direction", direction)?;
        }
        map.serialize_entry("nodes", &store.nodes)?;
        map.serialize_entry("edges", &store.edges)?;
        map.serialize_entry("subgraphs", &store.subgraphs)?;
        let connectors: IndexMap<&str, &AgentNode> = self
            .connectors
            .iter()
            .filter_map(|id| store.nodes.get(id).map(|node| (id.as_str(), node)))
            .collect();
        map.serialize_entry("connectors", &connectors)?;
        map.serialize_entry("classes", &store.classes)?;
        map.serialize_entry("tooltips", &store.tooltips)?;
        if let Some(style) = &store.default_edge_style {
            map.serialize_entry("defaultEdgeStyle", style)?;
        }
        if let Some(interpolate) = &store.default_interpolate {
            map.serialize_entry("defaultInterpolate", interpolate)?;
        }
        map.serialize_entry("mappings", &self.mappings)?;
        map.serialize_entry("diagnostics", &self.diagnostics)?;
        if let Some(title) = &store.titles.title {
            map.serialize_entry("title", title)?;
        }
        if let Some(acc_title) = &store.titles.acc_title {
            map.serialize_entry("accTitle", acc_title)?;
        }
        if let Some(acc_descr) = &store.titles.acc_descr {
            map.serialize_entry("accDescr", acc_descr)?;
        }
        map.serialize_entry("lineOffset", &self.options.line_offset)?;
        map.serialize_entry("subgraphLookup", &store.subgraphs_by_id())?;
        map.serialize_entry("globalNodes", &self.global_nodes)?;
        map.serialize_entry("members", &store.members)?;
        map.serialize_entry("edgeById", &store.edges_by_id())?;
        map.serialize_entry("pairCount", &store.pair_count)?;
        map.serialize_entry("subCount", &store.sub_count)?;
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
