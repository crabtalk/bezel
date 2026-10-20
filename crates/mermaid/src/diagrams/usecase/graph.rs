//! Resolves a parsed use case diagram into what is drawn: every actor, use
//! case, note, JSON table and boundary as a node with its class styles, and
//! every relationship and note connector as an edge with its markers. Ports
//! getData from Mermaid's use case DB. Pele's `diagrams/usecase/graph.ts`.

use super::json::pointer_key;
use crate::js::JsValue;
use indexmap::IndexMap;

use super::types::{
    ActorType, Animation, Arrow, BoundaryType, LabelType, Relationship, RelationshipType,
    UseCaseShape, UsecaseJsonNode, UsecaseModel,
};
use crate::{config::Value, js, svg::edges::Marker};

/// What an edge ends in: one of the shared markers, or the hollow triangle of
/// a generalization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Head {
    Marker(Marker),
    Extension,
}

impl Arrow {
    /// The heads at its start and its end.
    fn heads(self) -> (Option<Head>, Option<Head>) {
        let head = |marker| Some(Head::Marker(marker));
        match self {
            Arrow::Solid => (None, head(Marker::Arrow)),
            Arrow::Back => (head(Marker::Arrow), None),
            Arrow::Line => (None, None),
            Arrow::Circle => (None, head(Marker::Circle)),
            Arrow::Cross => (None, head(Marker::Cross)),
            Arrow::CircleReversed => (head(Marker::Circle), None),
            Arrow::CrossReversed => (head(Marker::Cross), None),
        }
    }
}

/// How a node is drawn, by Mermaid's shape names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeShape {
    /// `usecaseActor`, `usecaseActorHollow`, `usecaseActorAwesome` and
    /// `usecaseActorIcon`.
    Actor(ActorType),
    /// `rect`.
    Rect,
    /// `usecaseEllipse`.
    Ellipse,
    /// `usecaseBusiness`.
    BusinessEllipse,
    /// `note`.
    Note,
    /// `usecaseJsonTable`.
    JsonTable,
    /// `usecaseSystemBoundary`.
    SystemBoundary,
}

/// What an edge stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    Relationship(RelationshipType),
    /// The line from a note to what it is about.
    Note,
}

impl EdgeKind {
    pub fn name(self) -> &'static str {
        match self {
            EdgeKind::Relationship(kind) => kind.name(),
            EdgeKind::Note => "note",
        }
    }

    /// An include or an extend.
    pub fn is_semantic(self) -> bool {
        matches!(
            self,
            EdgeKind::Relationship(RelationshipType::Include | RelationshipType::Extend)
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct JsonRow {
    /// Blank on the second and later values of an array.
    pub key: String,
    /// The key again, for every row.
    pub accessible_key: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UsecaseGraphNode {
    pub id: String,
    pub label: String,
    pub label_type: LabelType,
    pub shape: NodeShape,
    pub is_group: bool,
    pub padding: f64,
    pub css_classes: String,
    pub css_styles: Vec<String>,
    pub css_compiled_styles: Vec<String>,
    pub actor_type: Option<ActorType>,
    pub business: Option<bool>,
    pub icon: Option<String>,
    pub stereotype: Option<String>,
    pub parent_id: Option<String>,
    pub note_target: Option<String>,
    pub json_rows: Option<Vec<JsonRow>>,
    pub boundary_type: Option<BoundaryType>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UsecaseGraphEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    pub kind: EdgeKind,
    pub dotted: bool,
    pub arrow_start: Option<Head>,
    pub arrow_end: Option<Head>,
    pub label: Option<String>,
    pub label_type: Option<LabelType>,
    pub classes: String,
    pub style: Vec<String>,
    pub css_compiled_styles: Vec<String>,
    pub animate: bool,
    pub animation: Option<Animation>,
    pub minlen: usize,
    pub explicit_id: bool,
    /// The dotted line from a note to what it is about.
    pub internal: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UsecaseGraph {
    pub nodes: Vec<UsecaseGraphNode>,
    pub edges: Vec<UsecaseGraphEdge>,
}

fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "null".into(),
        other => other.to_js_string(),
    }
}

impl UsecaseJsonNode {
    /// One row for each leaf of the object, depth first in the order the
    /// keys were written. Nesting is kept on a stack of its own, so depth is
    /// limited by memory and not by the call stack.
    pub fn rows(&self) -> Vec<JsonRow> {
        let mut rows: Vec<JsonRow> = Vec::new();
        let mut stack: Vec<(Option<&Value>, String, String)> =
            vec![(Some(&self.value), String::new(), String::new())];
        let mut children: Vec<(Option<&Value>, String, String)> = Vec::new();
        while let Some((current, path, pointer)) = stack.pop() {
            children.clear();
            let row = |key: &str, value: String| JsonRow {
                key: key.to_string(),
                accessible_key: path.clone(),
                value,
            };
            match current {
                Some(Value::Array(items)) => {
                    if items.is_empty() {
                        rows.push(row(&path, "[]".into()));
                    } else if items
                        .iter()
                        .all(|item| !matches!(item, Value::Array(_) | Value::Object(_)))
                    {
                        for (i, item) in items.iter().enumerate() {
                            rows.push(row(if i == 0 { &path } else { "" }, scalar(item)));
                        }
                    } else {
                        for (i, item) in items.iter().enumerate() {
                            children.push((
                                Some(item),
                                format!("{path}[{i}]"),
                                format!("{pointer}/{i}"),
                            ));
                        }
                    }
                }
                Some(Value::Object(object)) => {
                    let keys: Vec<String> = match self.property_order.get(&pointer) {
                        Some(keys) => keys.clone(),
                        None => object.iter().map(|(key, _)| key.to_string()).collect(),
                    };
                    if keys.is_empty() {
                        rows.push(row(&path, "{}".into()));
                    }
                    for key in keys {
                        children.push((
                            object.get(&key),
                            if path.is_empty() {
                                key.clone()
                            } else {
                                format!("{path}.{key}")
                            },
                            format!("{pointer}/{}", pointer_key(&key)),
                        ));
                    }
                }
                Some(other) => rows.push(row(&path, scalar(other))),
                None => rows.push(row(&path, "undefined".into())),
            }
            stack.extend(children.drain(..).rev());
        }
        rows
    }
}

fn class_list(names: &[Option<&str>]) -> String {
    names
        .iter()
        .flatten()
        .filter(|name| !name.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
}

impl Relationship {
    /// The heads at its start and its end.
    fn heads(&self) -> (Option<Head>, Option<Head>) {
        match self.kind {
            RelationshipType::Generalization => (None, Some(Head::Extension)),
            RelationshipType::Association => self.arrow_type.heads(),
            _ => Arrow::Solid.heads(),
        }
    }
}

impl UsecaseModel {
    /// A later class wins a property but keeps the place the property first
    /// took.
    fn compiled(&self, class_names: &[String]) -> Vec<String> {
        let mut out: IndexMap<String, String> = IndexMap::new();
        let names = std::iter::once("default").chain(class_names.iter().map(String::as_str));
        for name in names {
            let Some(definition) = self.class_defs.get(name) else {
                continue;
            };
            for raw in &definition.styles {
                let style = js::trim(raw);
                let property = js::trim(style.split_once(':').map_or(style, |(p, _)| p));
                if !property.is_empty() {
                    out.insert(property.to_string(), style.to_string());
                }
            }
        }
        out.into_values().collect()
    }

    pub fn graph(&self) -> UsecaseGraph {
        let mut nodes: Vec<UsecaseGraphNode> = Vec::new();
        let mut edges: Vec<UsecaseGraphEdge> = Vec::new();
        let some = |text: &Option<String>| text.clone().filter(|text| !text.is_empty());

        for actor in self.actors.values() {
            let kind_class = format!("usecase-actor-{}", actor.kind.name());
            let mut classes = vec![
                Some("default"),
                Some("usecase-actor"),
                Some(kind_class.as_str()),
                actor.business.then_some("usecase-business"),
            ];
            classes.extend(actor.classes.iter().map(|c| Some(c.as_str())));
            nodes.push(UsecaseGraphNode {
                id: actor.id.clone(),
                label: actor.label.clone(),
                label_type: actor.label_type,
                shape: NodeShape::Actor(actor.kind),
                is_group: false,
                padding: 10.0,
                css_classes: class_list(&classes),
                css_styles: actor.styles.clone(),
                css_compiled_styles: self.compiled(&actor.classes),
                actor_type: Some(actor.kind),
                business: Some(actor.business),
                icon: some(&actor.icon),
                stereotype: some(&actor.stereotype),
                parent_id: some(&actor.parent_id),
                note_target: None,
                json_rows: None,
                boundary_type: None,
            });
        }
        for use_case in self.use_cases.values() {
            let ellipse = use_case.shape == UseCaseShape::Ellipse;
            let shape_class = format!("usecase-{}", use_case.shape.name());
            let mut classes = vec![
                Some("default"),
                Some("usecase-element"),
                Some(shape_class.as_str()),
                use_case.business.then_some("usecase-business"),
            ];
            classes.extend(use_case.classes.iter().map(|c| Some(c.as_str())));
            nodes.push(UsecaseGraphNode {
                id: use_case.id.clone(),
                label: use_case.label.clone(),
                label_type: use_case.label_type,
                shape: if !ellipse {
                    NodeShape::Rect
                } else if use_case.business {
                    NodeShape::BusinessEllipse
                } else {
                    NodeShape::Ellipse
                },
                is_group: false,
                padding: if ellipse { 20.0 } else { 10.0 },
                css_classes: class_list(&classes),
                css_styles: use_case.styles.clone(),
                css_compiled_styles: self.compiled(&use_case.classes),
                actor_type: None,
                business: Some(use_case.business),
                icon: None,
                stereotype: some(&use_case.stereotype),
                parent_id: some(&use_case.parent_id),
                note_target: None,
                json_rows: None,
                boundary_type: None,
            });
        }
        for note in self.notes.values() {
            nodes.push(UsecaseGraphNode {
                id: note.id.clone(),
                label: note.label.clone(),
                label_type: note.label_type,
                shape: NodeShape::Note,
                is_group: false,
                padding: 10.0,
                css_classes: "default usecase-note".into(),
                css_styles: Vec::new(),
                css_compiled_styles: self.compiled(&[]),
                actor_type: None,
                business: None,
                icon: None,
                stereotype: None,
                parent_id: None,
                note_target: Some(note.target.clone()),
                json_rows: None,
                boundary_type: None,
            });
        }
        for json in self.json_nodes.values() {
            let mut classes = vec![Some("default"), Some("usecase-json-table")];
            classes.extend(json.classes.iter().map(|c| Some(c.as_str())));
            nodes.push(UsecaseGraphNode {
                id: json.id.clone(),
                label: json.id.clone(),
                label_type: LabelType::Text,
                shape: NodeShape::JsonTable,
                is_group: false,
                padding: 10.0,
                css_classes: class_list(&classes),
                css_styles: json.styles.clone(),
                css_compiled_styles: self.compiled(&json.classes),
                actor_type: None,
                business: None,
                icon: None,
                stereotype: None,
                parent_id: None,
                note_target: None,
                json_rows: Some(json.rows()),
                boundary_type: None,
            });
        }
        for boundary in self.system_boundaries.values() {
            let kind_class = format!("system-boundary-{}", boundary.kind.name());
            let mut classes = vec![
                Some("default"),
                Some("system-boundary"),
                Some(kind_class.as_str()),
            ];
            classes.extend(boundary.classes.iter().map(|c| Some(c.as_str())));
            nodes.push(UsecaseGraphNode {
                id: boundary.id.clone(),
                label: boundary.label.clone(),
                label_type: boundary.label_type,
                shape: NodeShape::SystemBoundary,
                is_group: true,
                padding: 20.0,
                css_classes: class_list(&classes),
                css_styles: boundary.styles.clone(),
                css_compiled_styles: self.compiled(&boundary.classes),
                actor_type: None,
                business: None,
                icon: None,
                stereotype: None,
                parent_id: None,
                note_target: None,
                json_rows: None,
                boundary_type: Some(boundary.kind),
            });
        }

        for relationship in &self.relationships {
            let semantic = matches!(
                relationship.kind,
                RelationshipType::Include | RelationshipType::Extend
            );
            let (arrow_start, arrow_end) = relationship.heads();
            let animated = relationship.animate || relationship.animation.is_some();
            let kind_class = format!("relationship-{}", relationship.kind.name());
            let animation_class = format!(
                "edge-animation-{}",
                relationship.animation.map_or("fast", Animation::name)
            );
            let mut classes = vec![
                Some("default"),
                Some("relationship"),
                Some(kind_class.as_str()),
            ];
            classes.extend(relationship.classes.iter().map(|c| Some(c.as_str())));
            classes.push(animated.then_some(animation_class.as_str()));
            let mut edge = UsecaseGraphEdge {
                id: relationship.id.clone(),
                start: relationship.source.clone(),
                end: relationship.target.clone(),
                kind: EdgeKind::Relationship(relationship.kind),
                dotted: semantic,
                arrow_start,
                arrow_end,
                label: None,
                label_type: None,
                classes: class_list(&classes),
                style: relationship.styles.clone(),
                css_compiled_styles: self.compiled(&relationship.classes),
                animate: relationship.animate,
                animation: relationship.animation,
                minlen: relationship.minlen,
                explicit_id: relationship.explicit_id,
                internal: false,
            };
            // Include and extend are labelled with their kind, whatever label
            // the relationship carries.
            if semantic {
                edge.label = Some(relationship.kind.name().to_string());
                edge.label_type = Some(LabelType::Text);
            } else if relationship.kind == RelationshipType::Association {
                if let Some(label) = relationship.label.as_ref().filter(|l| !l.is_empty()) {
                    edge.label = Some(label.clone());
                }
                if relationship.label_type.is_some() {
                    edge.label_type = relationship.label_type;
                }
            }
            edges.push(edge);
        }
        for note in self.notes.values() {
            edges.push(UsecaseGraphEdge {
                id: format!("{}-edge", note.id),
                start: note.id.clone(),
                end: note.target.clone(),
                kind: EdgeKind::Note,
                dotted: true,
                arrow_start: None,
                arrow_end: None,
                label: None,
                label_type: None,
                classes: "default relationship relationship-note".into(),
                style: Vec::new(),
                css_compiled_styles: self.compiled(&[]),
                animate: false,
                animation: None,
                minlen: 1,
                explicit_id: false,
                internal: true,
            });
        }
        UsecaseGraph { nodes, edges }
    }
}
