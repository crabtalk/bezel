//! Pele's `diagrams/state/graph.ts`: Mermaid's dataFetcher, which walks the
//! documents and resolves them into the nodes and edges that are drawn.

use std::collections::HashMap;

use indexmap::IndexMap;
use serde::Serialize;

use super::types::{Descr, StateClassDef, StateStmt, Stmt};
use crate::{detect::Kind, error::Error, js};

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateNode {
    pub id: String,
    /// rect, rectWithTitle, stateStart, stateEnd, choice, fork, join,
    /// divider, roundedWithTitle, note or noteGroup.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub description: Vec<String>,
    pub css_classes: String,
    pub css_compiled_styles: Vec<String>,
    pub css_styles: Vec<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub is_group: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// The direction inside a composite: its last `direction` statement, or
    /// TB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
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
    pub pattern: Option<&'static str>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct StateGraph {
    pub nodes: Vec<StateNode>,
    pub edges: Vec<StateEdge>,
}

struct Draft {
    shape: Option<String>,
    description: Descr,
    css_classes: String,
    css_styles: Vec<String>,
    kind: Option<String>,
    dir: Option<String>,
    color_index: Option<usize>,
}

struct Frame<'a> {
    item: Option<&'a StateStmt>,
    doc: &'a [Stmt],
    at: usize,
    alt: bool,
}

fn direction(doc: &[Stmt]) -> String {
    let mut dir = "TB";
    for item in doc {
        if let Stmt::Dir(item) = item {
            dir = &item.value;
        }
    }
    dir.to_string()
}

struct Builder<'a> {
    states: &'a IndexMap<String, StateStmt>,
    classes: &'a IndexMap<String, StateClassDef>,
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
            let mut shape = Some("rect".to_string());
            if item.start == Some(true) {
                shape = Some("stateStart".to_string());
            } else if item.start == Some(false) {
                shape = Some("stateEnd".to_string());
            }
            if item.kind.as_deref() != Some("default") {
                shape = item.kind.clone();
            }
            self.drafts.insert(
                id.to_string(),
                Draft {
                    shape,
                    description: Descr::One(id.to_string()),
                    css_classes: format!("{class_str} statediagram-state"),
                    css_styles: styles.clone(),
                    kind: None,
                    dir: None,
                    color_index: None,
                },
            );
        }
        let draft = self.drafts.get_mut(id).expect("a draft");

        if let Some(description) = item.description.as_ref().filter(|d| d.truthy()) {
            match &draft.description {
                Descr::Many(list) => {
                    draft.shape = Some("rectWithTitle".to_string());
                    let mut list = list.clone();
                    list.extend(description.texts());
                    draft.description = Descr::Many(list);
                }
                Descr::One(text) if !text.is_empty() => {
                    draft.shape = Some("rectWithTitle".to_string());
                    draft.description = if text == id {
                        Descr::Many(description.texts())
                    } else {
                        let mut list = vec![text.clone()];
                        list.extend(description.texts());
                        Descr::Many(list)
                    };
                }
                Descr::One(_) => {
                    draft.shape = Some("rect".to_string());
                    draft.description = description.clone();
                }
            }
        }
        let length = match &draft.description {
            Descr::One(text) => js::len(text),
            Descr::Many(list) => list.len(),
        };
        if length == 1 && draft.shape.as_deref() == Some("rectWithTitle") {
            draft.shape = Some(
                if draft.kind.as_deref() == Some("group") {
                    "roundedWithTitle"
                } else {
                    "rect"
                }
                .to_string(),
            );
        }

        if draft.kind.as_deref().is_none_or(str::is_empty)
            && let Some(doc) = &item.doc
        {
            draft.kind = Some("group".to_string());
            draft.dir = Some(direction(doc));
            draft.shape = Some(
                if item.kind.as_deref() == Some("divider") {
                    "divider"
                } else {
                    "roundedWithTitle"
                }
                .to_string(),
            );
            // A container takes the next palette slot unless it is a region,
            // which shares its composite's.
            let shared = parent.and_then(|parent| self.slots.get(&parent.id).copied());
            match shared {
                Some(slot) if draft.shape.as_deref() == Some("divider") => {
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
            shape: draft.shape.clone(),
            label,
            description,
            css_classes: draft.css_classes.clone(),
            css_compiled_styles: Vec::new(),
            css_styles: draft.css_styles.clone(),
            kind: draft.kind.clone(),
            is_group: draft.kind.as_deref() == Some("group"),
            dir: draft.dir.clone(),
            color_index: draft.color_index,
            ..StateNode::default()
        };
        if node.shape.as_deref() == Some("divider") {
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
            |aside_id: String, shape: &str, css_classes: String, is_group: bool| StateNode {
                id: aside_id,
                shape: Some(shape.to_string()),
                label: Some(note.text.clone()),
                css_classes,
                kind: Some(if is_group { "group" } else { "node" }.to_string()),
                is_group,
                position: note.position.clone(),
                ..StateNode::default()
            };
        let css_classes = draft.css_classes.clone();
        let mut note_node = aside(
            format!("{id}----note-{}", self.counter),
            "note",
            "statediagram-note".to_string(),
            false,
        );
        self.counter += 1;
        let group_id = format!("{id}----parent");
        note_node.parent_id = Some(group_id.clone());
        note_node.note_for = Some(id.to_string());
        let note_id = note_node.id.clone();
        self.insert(aside(group_id, "noteGroup", css_classes, true), true);
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
            pattern: Some("dashed"),
        });
    }
}

pub fn build_state_graph(
    root_doc: &[Stmt],
    states: &IndexMap<String, StateStmt>,
    classes: &IndexMap<String, StateClassDef>,
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
