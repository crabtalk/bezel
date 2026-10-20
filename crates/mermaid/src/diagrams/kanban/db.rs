//! Pele's `diagrams/kanban/db.ts`.

use crate::js::JsValue;
use serde::{Serialize, Serializer, ser::SerializeStruct};

use crate::{
    Error,
    config::Value,
    detect::Kind,
    diagrams::common::outline::{NodeType, OutlineDb},
    yaml::parse_yaml,
};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanNode {
    pub id: String,
    pub level: usize,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticket: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub css_classes: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct KanbanSection {
    #[serde(flatten)]
    pub node: KanbanNode,
    pub items: Vec<KanbanNode>,
}

/// Where a node is: a section, or an item of one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NodeRef {
    Section(usize),
    Item(usize, usize),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct KanbanModel {
    pub title: Option<String>,
    pub sections: Vec<KanbanSection>,
    last: Option<NodeRef>,
    section_level: usize,
    misplaced: Option<NodeRef>,
}

/// A node as Pele's model holds it: a section with its items.
enum Held<'a> {
    Section(&'a KanbanSection),
    Item(&'a KanbanNode),
}

impl Serialize for Held<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Held::Section(section) => section.serialize(s),
            Held::Item(node) => node.serialize(s),
        }
    }
}

impl Serialize for KanbanModel {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out = s.serialize_struct("KanbanModel", 6)?;
        out.serialize_field("type", "kanban")?;
        if let Some(title) = &self.title {
            out.serialize_field("title", title)?;
        }
        out.serialize_field("sections", &self.sections)?;
        if let Some(last) = self.last {
            out.serialize_field("last", &self.held(last))?;
        }
        out.serialize_field("sectionLevel", &self.section_level)?;
        if let Some(misplaced) = self.misplaced {
            out.serialize_field("misplaced", &self.held(misplaced))?;
        }
        out.end()
    }
}

impl KanbanModel {
    fn held(&self, at: NodeRef) -> Held<'_> {
        match at {
            NodeRef::Section(i) => Held::Section(&self.sections[i]),
            NodeRef::Item(i, k) => Held::Item(&self.sections[i].items[k]),
        }
    }

    fn node_mut(&mut self, at: NodeRef) -> &mut KanbanNode {
        match at {
            NodeRef::Section(i) => &mut self.sections[i].node,
            NodeRef::Item(i, k) => &mut self.sections[i].items[k],
        }
    }

    fn node(&self, at: NodeRef) -> &KanbanNode {
        match at {
            NodeRef::Section(i) => &self.sections[i].node,
            NodeRef::Item(i, k) => &self.sections[i].items[k],
        }
    }
}

/// `String(value)` for a truthy value.
fn given(value: Option<&Value>) -> Option<String> {
    value
        .filter(|value| value.truthy())
        .map(Value::to_js_string)
}

impl OutlineDb for KanbanModel {
    fn add_node(
        &mut self,
        level: usize,
        id: &str,
        descr: &str,
        _kind: NodeType,
        shape_data: Option<&str>,
    ) -> Result<(), Error> {
        let mut node = KanbanNode {
            id: id.to_string(),
            level,
            label: descr.to_string(),
            icon: None,
            assigned: None,
            ticket: None,
            priority: None,
            css_classes: None,
        };

        if let Some(shape_data) = shape_data {
            let yaml = if shape_data.contains('\n') {
                format!("{shape_data}\n")
            } else {
                format!("{{\n{shape_data}\n}}")
            };
            let doc = parse_yaml(&yaml).map_err(|error| {
                Error::syntax(format!(
                    "Metadata of \"{}\" is not valid YAML. {}",
                    node.id, error.0
                ))
                .of(Kind::Kanban)
            })?;
            // Mermaid throws a TypeError on metadata that is empty or not a
            // mapping. Here it is ignored.
            if let Value::Object(doc) = doc {
                let shape = given(doc.get("shape")).unwrap_or_default();
                if shape != shape.to_lowercase() || shape.contains('_') {
                    return Err(Error::semantic(format!(
                        "No such shape: {shape}. Shape names should be lowercase."
                    ))
                    .of(Kind::Kanban));
                }
                if let Some(label) = given(doc.get("label")) {
                    node.label = label;
                }
                if let Some(icon) = given(doc.get("icon")) {
                    node.icon = Some(icon);
                }
                if let Some(assigned) = given(doc.get("assigned")) {
                    node.assigned = Some(assigned);
                }
                if let Some(ticket) = given(doc.get("ticket")) {
                    node.ticket = Some(ticket);
                }
                if let Some(priority) = given(doc.get("priority")) {
                    node.priority = Some(priority);
                }
            }
        }

        // A node indented less than the first column becomes a card, and
        // Mermaid rejects whatever follows it.
        if let Some(misplaced) = self.misplaced {
            return Err(Error::semantic(format!(
                "Items without section detected, found section (\"{}\")",
                self.node(misplaced).label
            ))
            .of(Kind::Kanban));
        }
        if self.sections.is_empty() {
            self.section_level = level;
        }
        if level == self.section_level {
            self.sections.push(KanbanSection {
                node,
                items: Vec::new(),
            });
            self.last = Some(NodeRef::Section(self.sections.len() - 1));
        } else {
            let i = self.sections.len() - 1;
            self.sections[i].items.push(node);
            let at = NodeRef::Item(i, self.sections[i].items.len() - 1);
            self.last = Some(at);
            if level < self.section_level {
                self.misplaced = Some(at);
            }
        }
        Ok(())
    }

    /// Mermaid throws a TypeError when a decoration comes before any node.
    /// Here it is ignored.
    fn decorate_node(&mut self, icon: Option<&str>, class: Option<&str>) {
        let Some(last) = self.last else {
            return;
        };
        let node = self.node_mut(last);
        if let Some(icon) = icon.filter(|icon| !icon.is_empty()) {
            node.icon = Some(icon.to_string());
        }
        if let Some(class) = class.filter(|class| !class.is_empty()) {
            node.css_classes = Some(class.to_string());
        }
    }
}
