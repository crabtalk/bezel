//! Pele's `diagrams/state/graph.ts`: Mermaid's dataFetcher, which walks the
//! documents and resolves them into the nodes and edges that are drawn.

use std::collections::HashMap;

use indexmap::IndexMap;
use serde::Serialize;

use super::{
    db::last_direction,
    types::{Descr, StateKind, StateStmt, Stmt},
};
use crate::diagrams::common::style_class::StyleClass;
use crate::{detect::Kind, error::Error, geom::Direction, js};

/// What a node of the graph is drawn as, by Mermaid's names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StateShape {
    Rect,
    RectWithTitle,
    StateStart,
    StateEnd,
    Choice,
    Fork,
    Join,
    Divider,
    RoundedWithTitle,
    Note,
    NoteGroup,
}

/// Whether a node holds others.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Group,
    Node,
}

/// How an edge is drawn when not as a transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Pattern {
    /// The edge between a note and its state.
    Dashed,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateNode {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<StateShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub description: Vec<String>,
    pub css_classes: String,
    pub css_compiled_styles: Vec<String>,
    pub css_styles: Vec<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<NodeKind>,
    pub is_group: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// The direction inside a composite: its last `direction` statement, or
    /// TB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<Direction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_for: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_index: Option<usize>,
}

/// Labels are markdown throughout, as in Mermaid.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StateEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    pub label: String,
    pub classes: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<Pattern>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct StateGraph {
    pub nodes: Vec<StateNode>,
    pub edges: Vec<StateEdge>,
}

struct Draft {
    shape: Option<StateShape>,
    description: Descr,
    css_classes: String,
    css_styles: Vec<String>,
    is_group: bool,
    dir: Option<Direction>,
    color_index: Option<usize>,
}

struct Frame<'a> {
    item: Option<&'a StateStmt>,
    doc: &'a [Stmt],
    at: usize,
    alt: bool,
}

struct Builder<'a> {
    states: &'a IndexMap<String, StateStmt>,
    classes: &'a IndexMap<String, StyleClass>,
    nodes: Vec<StateNode>,
    edges: Vec<StateEdge>,
    by_id: HashMap<String, usize>,
    drafts: HashMap<String, Draft>,
    slots: HashMap<String, Option<usize>>,
    counter: usize,
    next_slot: usize,
}

impl Builder<'_> {
    /// `aside` for a note and its group, which carry no `colorIndex` and do
    /// carry a `position` when merged into a node already there.
    fn insert(&mut self, mut node: StateNode, aside: bool) {
        // The two literal ids are what DOMPurify leaves of `<<fork>>` and
        // `<<choice>>` in Mermaid; kept for parity.
        if node.id.is_empty() || node.id == "</join></fork>" || node.id == "</choice>" {
            return;
        }
        if !node.css_classes.is_empty() {
            // One copy, however many classes: a state named in a class
            // statement many times has a long list.
            let mut compiled: Option<Vec<String>> = None;
            for name in node.css_classes.split(' ') {
                let Some(def) = self.classes.get(name) else {
                    continue;
                };
                let list = compiled.get_or_insert_with(|| node.css_compiled_styles.clone());
                list.extend(def.styles.iter().cloned());
            }
            if let Some(compiled) = compiled {
                node.css_compiled_styles = compiled;
            }
        }
        match self.by_id.get(&node.id) {
            Some(&at) => {
                let existing = &mut self.nodes[at];
                let parent_id = node.parent_id.clone().or(existing.parent_id.take());
                // Object.assign: every key the new node has.
                let (position, note_for, color_index) = (
                    existing.position.take(),
                    existing.note_for.take(),
                    existing.color_index,
                );
                let had_note_for = node.note_for.is_some();
                *existing = node;
                if aside {
                    existing.color_index = color_index;
                    if !had_note_for {
                        existing.note_for = note_for;
                    }
                } else {
                    existing.position = position;
                    existing.note_for = note_for;
                }
                existing.parent_id = parent_id;
            }
            None => {
                self.by_id.insert(node.id.clone(), self.nodes.len());
                self.nodes.push(node);
            }
        }
    }

    fn visit(&mut self, parent: Option<&StateStmt>, item: &StateStmt, alt: bool) {
        let id = item.id.as_str();
        if id == "root" {
            return;
        }
        let known = self.states.get(id);
        let class_str = known
            .and_then(|known| known.classes.as_ref())
            .map_or(String::new(), |classes| classes.join(" "));
        let styles = known
            .and_then(|known| known.styles.clone())
            .unwrap_or_default();

        if !self.drafts.contains_key(id) {
            let mut shape = Some(match item.start {
                Some(true) => StateShape::StateStart,
                Some(false) => StateShape::StateEnd,
                None => StateShape::Rect,
            });
            if item.kind != Some(StateKind::Default) {
                shape = item.kind.map(StateKind::shape);
            }
            self.drafts.insert(
                id.to_string(),
                Draft {
                    shape,
                    description: Descr::One(id.to_string()),
                    css_classes: format!("{class_str} statediagram-state"),
                    css_styles: styles.clone(),
                    is_group: false,
                    dir: None,
                    color_index: None,
                },
            );
        }
        let draft = self.drafts.get_mut(id).expect("a draft");

        if let Some(description) = item.description.as_ref().filter(|d| !d.is_empty()) {
            match &draft.description {
                Descr::Many(list) => {
                    draft.shape = Some(StateShape::RectWithTitle);
                    let mut list = list.clone();
                    list.extend(description.texts());
                    draft.description = Descr::Many(list);
                }
                Descr::One(text) if !text.is_empty() => {
                    draft.shape = Some(StateShape::RectWithTitle);
                    draft.description = if text == id {
                        Descr::Many(description.texts())
                    } else {
                        let mut list = vec![text.clone()];
                        list.extend(description.texts());
                        Descr::Many(list)
                    };
                }
                Descr::One(_) => {
                    draft.shape = Some(StateShape::Rect);
                    draft.description = description.clone();
                }
            }
        }
        let length = match &draft.description {
            Descr::One(text) => js::len(text),
            Descr::Many(list) => list.len(),
        };
        if length == 1 && draft.shape == Some(StateShape::RectWithTitle) {
            draft.shape = Some(if draft.is_group {
                StateShape::RoundedWithTitle
            } else {
                StateShape::Rect
            });
        }

        if !draft.is_group
            && let Some(doc) = &item.doc
        {
            draft.is_group = true;
            draft.dir = Some(last_direction(doc));
            draft.shape = Some(if item.kind == Some(StateKind::Divider) {
                StateShape::Divider
            } else {
                StateShape::RoundedWithTitle
            });
            // A container takes the next palette slot unless it is a region,
            // which shares its composite's.
            let shared = parent.and_then(|parent| self.slots.get(&parent.id).copied());
            match shared {
                Some(slot) if draft.shape == Some(StateShape::Divider) => {
                    draft.color_index = slot;
                }
                _ => {
                    let slot = self.next_slot;
                    self.next_slot += 1;
                    draft.color_index = if !js::trim(&class_str).is_empty() || !styles.is_empty() {
                        None
                    } else {
                        Some(slot)
                    };
                }
            }
            self.slots.insert(id.to_string(), draft.color_index);
            draft.css_classes = format!(
                "{} statediagram-cluster {}",
                draft.css_classes,
                if alt { "statediagram-cluster-alt" } else { "" }
            );
        }

        let (label, description) = match &draft.description {
            Descr::Many(list) => (
                list.first().cloned(),
                list.iter().skip(1).cloned().collect(),
            ),
            Descr::One(text) => (Some(text.clone()), Vec::new()),
        };
        let mut node = StateNode {
            id: id.to_string(),
            shape: draft.shape,
            label,
            description,
            css_classes: draft.css_classes.clone(),
            css_compiled_styles: Vec::new(),
            css_styles: draft.css_styles.clone(),
            kind: draft.is_group.then_some(NodeKind::Group),
            is_group: draft.is_group,
            dir: draft.dir,
            color_index: draft.color_index,
            ..StateNode::default()
        };
        if node.shape == Some(StateShape::Divider) {
            node.label = Some(String::new());
        }
        if let Some(parent) = parent
            && parent.id != "root"
        {
            node.parent_id = Some(parent.id.clone());
        }

        let Some(note) = &item.note else {
            self.insert(node, false);
            return;
        };
        // Mermaid puts each note in a group of its own, named after the
        // state. Pele keeps that group in the model and lays the note out
        // beside its state instead.
        let aside =
            |aside_id: String, shape: StateShape, css_classes: String, is_group: bool| StateNode {
                id: aside_id,
                shape: Some(shape),
                label: Some(note.text.clone()),
                css_classes,
                kind: Some(if is_group {
                    NodeKind::Group
                } else {
                    NodeKind::Node
                }),
                is_group,
                position: note.position.clone(),
                ..StateNode::default()
            };
        let css_classes = draft.css_classes.clone();
        let mut note_node = aside(
            format!("{id}----note-{}", self.counter),
            StateShape::Note,
            "statediagram-note".to_string(),
            false,
        );
        self.counter += 1;
        let group_id = format!("{id}----parent");
        note_node.parent_id = Some(group_id.clone());
        note_node.note_for = Some(id.to_string());
        let note_id = note_node.id.clone();
        self.insert(
            aside(group_id, StateShape::NoteGroup, css_classes, true),
            true,
        );
        self.insert(note_node, true);
        self.insert(node, false);
        let left = note.position.as_deref() == Some("left of");
        let (from, to) = if left {
            (note_id, id.to_string())
        } else {
            (id.to_string(), note_id)
        };
        self.edges.push(StateEdge {
            id: format!("{from}-{to}"),
            start: from,
            end: to,
            label: String::new(),
            classes: "transition note-edge".to_string(),
            pattern: Some(Pattern::Dashed),
        });
    }
}

pub fn build_state_graph(
    root_doc: &[Stmt],
    states: &IndexMap<String, StateStmt>,
    classes: &IndexMap<String, StyleClass>,
) -> Result<StateGraph, Error> {
    let mut b = Builder {
        states,
        classes,
        nodes: Vec::new(),
        edges: Vec::new(),
        by_id: HashMap::new(),
        drafts: HashMap::new(),
        slots: HashMap::new(),
        counter: 0,
        next_slot: 0,
    };

    let mut stack = vec![Frame {
        item: None,
        doc: root_doc,
        at: 0,
        alt: true,
    }];
    while let Some(frame) = stack.last_mut() {
        if frame.at >= frame.doc.len() {
            stack.pop();
            continue;
        }
        let item = &frame.doc[frame.at];
        frame.at += 1;
        let (parent, alt) = (frame.item, frame.alt);
        match item {
            Stmt::State(item) => {
                b.visit(parent, item, alt);
                if let Some(doc) = &item.doc {
                    stack.push(Frame {
                        item: Some(item),
                        doc,
                        at: 0,
                        alt: !alt,
                    });
                }
            }
            Stmt::Relation(item) => {
                b.visit(parent, &item.state1, alt);
                b.visit(parent, &item.state2, alt);
                b.edges.push(StateEdge {
                    id: format!("edge{}", b.counter),
                    start: item.state1.id.clone(),
                    end: item.state2.id.clone(),
                    label: item.description.clone().unwrap_or_default(),
                    classes: "transition".to_string(),
                    pattern: None,
                });
                b.counter += 1;
            }
            _ => {}
        }
    }

    for node in &b.nodes {
        if node.is_group && !node.description.is_empty() {
            return Err(Error::semantic(format!(
                "Group nodes can only have label. Remove the additional description for node [{}]",
                node.id
            ))
            .of(Kind::State));
        }
    }
    Ok(StateGraph {
        nodes: b.nodes,
        edges: b.edges,
    })
}
