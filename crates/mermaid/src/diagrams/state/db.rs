//! Pele's `diagrams/state/db.ts`: Mermaid's StateDB in its version 2 form,
//! which is what Mermaid uses for both `stateDiagram` and
//! `stateDiagram-v2`. The legacy version, which numbers `[*]` states, is not
//! ported.

use indexmap::IndexMap;

use super::{
    graph::build_state_graph,
    types::{Descr, StateKind, StateLink, StateModel, StateNote, StateRelation, StateStmt, Stmt},
};
use crate::{diagrams::common::style_class::StyleClass, error::Error, geom::Direction, js};

/// A document being given ids, taken out of its state while its statements
/// are visited.
struct Frame {
    id: String,
    doc: Vec<Stmt>,
    at: usize,
}

impl StateModel {
    pub fn new() -> Self {
        Self {
            kind: "state",
            ..Self::default()
        }
    }

    /// The direction the root document is laid out in: its last `direction`
    /// statement.
    pub fn direction(&self) -> Direction {
        last_direction(&self.root_doc)
    }

    pub fn set_root_doc(&mut self, doc: Vec<Stmt>) -> Result<(), Error> {
        self.root_doc = doc;
        self.translate();
        let doc = std::mem::take(&mut self.root_doc);
        let result = self.extract(&doc);
        self.root_doc = doc;
        result
    }

    /// Only the root document's statements become states, relations and
    /// classes here; the statements inside composites are read when the graph
    /// is built.
    pub fn extract(&mut self, doc: &[Stmt]) -> Result<(), Error> {
        self.states = IndexMap::new();
        self.relations = Vec::new();
        self.classes = IndexMap::new();
        for item in doc {
            match item {
                Stmt::State(item) => self.add_state(
                    js::trim(&item.id),
                    item.kind,
                    item.doc.as_ref(),
                    item.description.as_ref(),
                    item.note.as_ref(),
                    None,
                ),
                Stmt::Relation(item) => {
                    self.add_relation(&item.state1, &item.state2, item.description.as_deref())
                }
                Stmt::ClassDef(item) => self.add_style_class(js::trim(&item.id), &item.classes),
                Stmt::Style(item) => {
                    let styles: Vec<String> = item
                        .style_class
                        .split(',')
                        .map(|s| js::trim(&s.replace(';', "")).to_string())
                        .collect();
                    for key in self.named(js::trim(&item.id)) {
                        if let Some(state) = self.states.get_mut(&key) {
                            state.styles = Some(styles.clone());
                        }
                    }
                }
                Stmt::ApplyClass(item) => self.set_css_class(js::trim(&item.id), &item.style_class),
                Stmt::Click(item) => self.add_link(&item.id.id, &item.url, &item.tooltip),
                _ => {}
            }
        }
        self.graph = build_state_graph(doc, &self.states, &self.classes)?;
        Ok(())
    }

    /// Mermaid's docTranslator: gives each `[*]` an id scoped to its parent,
    /// and turns a document split by `--` into one region per part.
    pub fn translate(&mut self) {
        let mut root = std::mem::take(&mut self.root_doc);
        self.regions(&mut root);
        let mut stack = vec![Frame {
            id: "root".to_string(),
            doc: root,
            at: 0,
        }];
        loop {
            let top = stack.last_mut().expect("the root frame");
            if top.at >= top.doc.len() {
                let done = stack.pop().expect("a frame");
                match stack.last_mut() {
                    Some(parent) => {
                        if let Stmt::State(state) = &mut parent.doc[parent.at - 1] {
                            state.doc = Some(done.doc);
                        }
                    }
                    None => {
                        self.root_doc = done.doc;
                        return;
                    }
                }
                continue;
            }
            let parent_id = top.id.clone();
            let at = top.at;
            top.at += 1;
            match &mut top.doc[at] {
                Stmt::Relation(relation) => {
                    relation.state1.name_in(&parent_id, true);
                    relation.state2.name_in(&parent_id, false);
                }
                Stmt::State(state) => {
                    state.name_in(&parent_id, true);
                    if let Some(mut doc) = state.doc.take() {
                        let id = state.id.clone();
                        self.regions(&mut doc);
                        stack.push(Frame { id, doc, at: 0 });
                    }
                }
                _ => {}
            }
        }
    }

    /// A document split by dividers, as one region per part.
    fn regions(&mut self, doc: &mut Vec<Stmt>) {
        let mut regions: Vec<Stmt> = Vec::new();
        let mut current: Vec<Stmt> = Vec::new();
        for item in std::mem::take(doc) {
            match item {
                Stmt::State(mut state)
                    if state.kind == Some(StateKind::Divider) && state.doc.is_none() =>
                {
                    state.doc = Some(std::mem::take(&mut current));
                    regions.push(Stmt::State(state));
                }
                item => current.push(item),
            }
        }
        // Mermaid names the region after the last divider with a random id;
        // here it continues the divider numbering. Mermaid also leaves a
        // document that ends with a divider ungrouped and then fails to draw
        // it; here the regions before that divider are kept.
        if regions.is_empty() {
            *doc = current;
            return;
        }
        if !current.is_empty() {
            let mut state = StateStmt::new(self.next_divider_id(), Some(StateKind::Divider));
            state.doc = Some(current);
            regions.push(Stmt::State(Box::new(state)));
        }
        *doc = regions;
    }

    pub fn add_state(
        &mut self,
        id: &str,
        kind: Option<StateKind>,
        doc: Option<&Vec<Stmt>>,
        descr: Option<&Descr>,
        note: Option<&StateNote>,
        classes: Option<&Vec<String>>,
    ) {
        let trimmed = js::trim(id).to_string();
        let kind = kind.unwrap_or(StateKind::Default);
        match self.states.get_mut(&trimmed) {
            None => {
                self.states.insert(
                    trimmed.clone(),
                    StateStmt {
                        id: trimmed.clone(),
                        descriptions: Some(Vec::new()),
                        kind: Some(kind),
                        doc: doc.cloned(),
                        note: note.cloned(),
                        classes: Some(Vec::new()),
                        styles: Some(Vec::new()),
                        text_styles: Some(Vec::new()),
                        ..StateStmt::default()
                    },
                );
            }
            Some(state) => {
                if state.doc.is_none() {
                    state.doc = doc.cloned();
                }
                if state.kind.is_none() {
                    state.kind = Some(kind);
                }
            }
        }
        if let Some(descr) = descr.filter(|descr| !descr.is_empty()) {
            for text in descr.texts() {
                self.add_description(&trimmed, js::trim(&text));
            }
        }
        if let Some(note) = note
            && let Some(state) = self.states.get_mut(&trimmed)
        {
            state.note = Some(note.clone());
        }
        if let Some(classes) = classes {
            for name in classes {
                self.set_css_class(&trimmed, js::trim(name));
            }
        }
    }

    pub fn add_link(&mut self, state_id: &str, url: &str, tooltip: &str) {
        self.links.insert(
            state_id.to_string(),
            StateLink {
                url: url.to_string(),
                tooltip: tooltip.to_string(),
            },
        );
    }

    pub fn add_relation(&mut self, item1: &StateStmt, item2: &StateStmt, title: Option<&str>) {
        for item in [item1, item2] {
            self.add_state(
                &item.id,
                item.kind,
                item.doc.as_ref(),
                item.description.as_ref(),
                item.note.as_ref(),
                item.classes.as_ref(),
            );
        }
        self.relations.push(StateRelation {
            id1: js::trim(&item1.id).to_string(),
            id2: js::trim(&item2.id).to_string(),
            relation_title: title.unwrap_or("").to_string(),
        });
    }

    pub fn add_description(&mut self, id: &str, descr: &str) {
        if let Some(descriptions) = self
            .states
            .get_mut(id)
            .and_then(|state| state.descriptions.as_mut())
        {
            descriptions.push(if descr.starts_with(':') {
                js::trim(&descr.replacen(':', "", 1)).to_string()
            } else {
                descr.to_string()
            });
        }
    }

    pub fn next_divider_id(&mut self) -> String {
        self.divider_count += 1;
        format!("divider-id-{}", self.divider_count)
    }

    pub fn add_style_class(&mut self, id: &str, style_attributes: &str) {
        let found = self
            .classes
            .entry(id.to_string())
            .or_insert_with(|| StyleClass::new(id));
        if style_attributes.is_empty() {
            return;
        }
        for attribute in style_attributes.split(',') {
            found.push_attribute(attribute);
        }
    }

    pub fn set_css_class(&mut self, item_ids: &str, css_class_name: &str) {
        for key in self.named(item_ids) {
            if let Some(classes) = self
                .states
                .get_mut(&key)
                .and_then(|state| state.classes.as_mut())
            {
                classes.push(css_class_name.to_string());
            }
        }
    }

    /// The keys of the states a comma-separated list of ids names, created
    /// where they do not exist yet. As in Mermaid, an id is looked up as
    /// written and trimmed only when it has to be created.
    fn named(&mut self, ids: &str) -> Vec<String> {
        let mut out = Vec::new();
        for id in ids.split(',') {
            if self.states.contains_key(id) {
                out.push(id.to_string());
                continue;
            }
            self.add_state(id, None, None, None, None, None);
            let trimmed = js::trim(id);
            if self.states.contains_key(trimmed) {
                out.push(trimmed.to_string());
            }
        }
        out
    }
}

impl StateStmt {
    /// Names a `[*]` after the document `parent_id`, as its start or its end;
    /// trims any other id.
    fn name_in(&mut self, parent_id: &str, start: bool) {
        if self.id == "[*]" {
            self.id = format!("{parent_id}{}", if start { "_start" } else { "_end" });
            self.start = Some(start);
        } else {
            self.id = js::trim(&self.id).to_string();
        }
    }
}

/// The last `direction` statement of `doc`, or top to bottom.
pub(super) fn last_direction(doc: &[Stmt]) -> Direction {
    doc.iter()
        .rev()
        .find_map(|item| match item {
            Stmt::Dir(item) => Some(item.value),
            _ => None,
        })
        .unwrap_or_default()
}
