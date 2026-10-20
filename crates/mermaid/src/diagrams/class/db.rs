//! Pele's `diagrams/class/db.ts`.

use indexmap::{IndexMap, IndexSet};

use crate::geom::Direction;
use serde::{
    Serialize, Serializer,
    ser::{SerializeMap, SerializeStruct},
};

use super::{
    members::{ClassMember, MemberType},
    types::{
        ClassInterface, ClassNode, ClassNote, ClassRelation, LineType, NamespaceNode, RelationEnd,
        RelationType, StyleClass,
    },
};
use crate::{diagrams::common::db::Titles, js, text::decode_entities, util::url::sanitize_url};

/// Mermaid looks up ids that start with a digit under this prefix, so they
/// never match a class.
const DOM_ID_PREFIX: &str = "classId-";

/// The class name and the generic type of `id`, split at its tildes.
fn split_name(id: &str) -> (&str, &str) {
    match id.find('~') {
        None | Some(0) => (id, ""),
        Some(at) => {
            let end = id[at + 1..].find('~').map_or(id.len(), |e| at + 1 + e);
            (&id[..at], &id[at + 1..end])
        }
    }
}

fn lookup_id(id: &str) -> String {
    if id.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        split_name(&format!("{DOM_ID_PREFIX}{id}")).0.to_string()
    } else {
        split_name(id).0.to_string()
    }
}

/// Pele's `ClassDb`, serialized with its bookkeeping as Pele's is.
#[derive(Clone, Debug, PartialEq)]
pub struct ClassModel {
    pub direction: Direction,
    pub classes: IndexMap<String, ClassNode>,
    pub relations: Vec<ClassRelation>,
    pub notes: IndexMap<String, ClassNote>,
    pub namespaces: IndexMap<String, NamespaceNode>,
    pub interfaces: Vec<ClassInterface>,
    pub style_classes: IndexMap<String, StyleClass>,
    pub titles: Titles,
    namespace_stack: Vec<String>,
    /// The classes tagged with each CSS class name, by id.
    with_css_class: IndexMap<String, IndexSet<String>>,
}

impl Default for ClassModel {
    fn default() -> Self {
        Self {
            direction: Direction::TopToBottom,
            classes: IndexMap::new(),
            relations: Vec::new(),
            notes: IndexMap::new(),
            namespaces: IndexMap::new(),
            interfaces: Vec::new(),
            style_classes: IndexMap::new(),
            titles: Titles::default(),
            namespace_stack: Vec::new(),
            with_css_class: IndexMap::new(),
        }
    }
}

impl ClassModel {
    pub fn add_class(&mut self, id: &str) {
        let (class_name, kind) = split_name(id);
        if self.classes.contains_key(class_name) {
            return;
        }
        self.classes.insert(
            class_name.to_string(),
            ClassNode {
                id: class_name.to_string(),
                kind: kind.to_string(),
                label: class_name.to_string(),
                css_classes: "default".into(),
                methods: Vec::new(),
                members: Vec::new(),
                annotations: Vec::new(),
                styles: Vec::new(),
                parent: None,
                link: None,
                link_target: None,
                have_callback: None,
                tooltip: None,
            },
        );
        self.tag(class_name, "default");
    }

    fn tag(&mut self, node: &str, css_class: &str) {
        self.with_css_class
            .entry(css_class.to_string())
            .or_default()
            .insert(node.to_string());
    }

    /// The class for a name that may carry a generic type, created if
    /// needed. Mermaid throws a TypeError where this creates, as for an
    /// annotation on a class not yet declared.
    fn ensure(&mut self, id: &str) -> &mut ClassNode {
        self.add_class(id);
        self.classes
            .get_mut(split_name(id).0)
            .expect("add_class made it")
    }

    pub fn set_class_label(&mut self, id: &str, label: &str) {
        self.ensure(id).label = label.to_string();
    }

    pub fn add_relation(&mut self, mut relation: ClassRelation) {
        let (type1, type2) = (relation.relation.type1, relation.relation.type2);
        let lollipop = RelationEnd::Type(RelationType::Lollipop);
        if type1 == lollipop && type2 == RelationEnd::None {
            self.add_class(&relation.id2);
            relation.id1 = self.add_interface(&relation.id1, &relation.id2);
        } else if type2 == lollipop && type1 == RelationEnd::None {
            self.add_class(&relation.id1);
            relation.id2 = self.add_interface(&relation.id2, &relation.id1);
        } else {
            self.add_class(&relation.id1);
            self.add_class(&relation.id2);
        }
        relation.id1 = split_name(&relation.id1).0.to_string();
        relation.id2 = split_name(&relation.id2).0.to_string();
        relation.relation_title1 = js::trim(&relation.relation_title1).to_string();
        relation.relation_title2 = js::trim(&relation.relation_title2).to_string();
        self.relations.push(relation);
    }

    fn add_interface(&mut self, label: &str, class_id: &str) -> String {
        let id = format!("interface{}", self.interfaces.len());
        self.interfaces.push(ClassInterface {
            id: id.clone(),
            label: label.to_string(),
            class_id: class_id.to_string(),
        });
        id
    }

    pub fn add_annotation(&mut self, class_name: &str, annotation: &str) {
        self.ensure(class_name)
            .annotations
            .push(annotation.to_string());
    }

    pub fn add_member(&mut self, class_name: &str, member: &str) {
        let node = self.ensure(class_name);
        let text = js::trim(member);
        if let Some(annotation) = text
            .strip_prefix("<<")
            .and_then(|text| text.strip_suffix(">>"))
        {
            node.annotations.push(annotation.to_string());
        } else if text.find(')').is_some_and(|at| at > 0) {
            let parsed = ClassMember::new(text, MemberType::Method);
            if parsed.member_type == MemberType::Method {
                node.methods.push(parsed);
            } else {
                node.members.push(parsed);
            }
        } else if !text.is_empty() {
            node.members
                .push(ClassMember::new(text, MemberType::Attribute));
        }
    }

    pub fn add_note(&mut self, text: &str, class_name: Option<&str>) -> String {
        let index = self.notes.len();
        let id = format!("note{index}");
        self.notes.insert(
            id.clone(),
            ClassNote {
                id: id.clone(),
                class: class_name.map(str::to_string),
                text: text.to_string(),
                index,
                parent: None,
            },
        );
        id
    }

    pub fn set_css_class(&mut self, ids: &str, class_name: &str) {
        for id in ids.split(',') {
            let key = lookup_id(id);
            let Some(node) = self.classes.get_mut(&key) else {
                continue;
            };
            node.css_classes.push(' ');
            node.css_classes.push_str(class_name);
            let node_id = node.id.clone();
            for word in class_name.split(' ') {
                self.tag(&node_id, word);
            }
        }
    }

    pub fn define_class(&mut self, ids: &[&str], style: &[String]) {
        for &id in ids {
            let style_class = self
                .style_classes
                .entry(id.to_string())
                .or_insert_with(|| StyleClass::new(id));
            for s in style {
                style_class.push(s);
            }
            // Mermaid tests `cssClasses.includes(id)`, which also matches part
            // of a longer name (a classDef named `def` styles every class,
            // through `default`). Whole names are compared here.
            let Some(tagged) = self.with_css_class.get(id) else {
                continue;
            };
            let parts: Vec<&str> = style.iter().flat_map(|s| s.split(',')).collect();
            for node in tagged {
                if let Some(node) = self.classes.get_mut(node) {
                    node.styles
                        .extend(parts.iter().map(|part| part.to_string()));
                }
            }
        }
    }

    pub fn set_tooltip(&mut self, ids: &str, tooltip: &str) {
        for id in ids.split(',') {
            if let Some(node) = self.classes.get_mut(split_name(id).0) {
                node.tooltip = Some(tooltip.to_string());
            }
        }
    }

    pub fn set_link(&mut self, ids: &str, link: &str, target: Option<&str>) {
        // Entity codes are resolved first, so that what is checked is what a
        // browser would follow.
        let url = js::trim(&decode_entities(link)).to_string();
        for id in ids.split(',') {
            if let Some(node) = self.classes.get_mut(&lookup_id(id)) {
                node.link = (!url.is_empty()).then(|| sanitize_url(&url));
                node.link_target = Some(target.unwrap_or("_blank").to_string());
            }
        }
        self.set_css_class(ids, "clickable");
    }

    /// Callbacks are never executed; the class is only marked clickable, as
    /// in Mermaid's strict mode.
    pub fn set_click_event(&mut self, ids: &str) {
        for id in ids.split(',') {
            if let Some(node) = self.classes.get_mut(split_name(id).0) {
                node.have_callback = Some(true);
            }
        }
        self.set_css_class(ids, "clickable");
    }

    pub fn add_namespace(&mut self, id: &str, label: Option<&str>) -> String {
        let prefix = self.namespace_stack.last().cloned().unwrap_or_default();
        let qualified = if prefix.is_empty() {
            id.to_string()
        } else {
            format!("{prefix}.{id}")
        };
        self.namespace_stack.push(qualified.clone());
        let label = label.filter(|label| !label.is_empty());

        if let Some(existing) = self.namespaces.get_mut(&qualified) {
            existing.explicit = true;
            if let Some(label) = label {
                existing.label = label.to_string();
            }
            return qualified;
        }

        // The enclosing namespace and its ancestors exist already, so only
        // the parts of `id` are walked.
        let mut parent_id = (!prefix.is_empty()).then(|| prefix.clone());
        let mut from = if prefix.is_empty() {
            0
        } else {
            prefix.len() + 1
        };
        loop {
            let dot = qualified[from..].find('.').map(|at| from + at);
            let leaf = dot.is_none();
            let current = &qualified[..dot.unwrap_or(qualified.len())];
            match self.namespaces.get_mut(current) {
                None => {
                    let part = &qualified[from..dot.unwrap_or(qualified.len())];
                    self.namespaces.insert(
                        current.to_string(),
                        NamespaceNode {
                            id: current.to_string(),
                            label: match label {
                                Some(label) if leaf => label.to_string(),
                                _ => part.to_string(),
                            },
                            classes: IndexSet::new(),
                            notes: IndexSet::new(),
                            children: IndexSet::new(),
                            parent: parent_id.clone(),
                            explicit: leaf,
                        },
                    );
                }
                Some(node) => {
                    if leaf {
                        node.explicit = true;
                    }
                }
            }
            if let Some(parent) = parent_id.as_deref()
                && self.namespaces.contains_key(parent)
            {
                self.namespaces[parent].children.insert(current.to_string());
                let node = &mut self.namespaces[current];
                if node.parent.is_none() {
                    node.parent = Some(parent.to_string());
                }
            }
            let Some(dot) = dot else {
                return qualified;
            };
            parent_id = Some(current.to_string());
            from = dot + 1;
        }
    }

    pub fn pop_namespace(&mut self) {
        self.namespace_stack.pop();
    }

    pub fn add_classes_to_namespace(
        &mut self,
        id: &str,
        class_names: &[String],
        note_names: &[String],
    ) {
        if !self.namespaces.contains_key(id) {
            return;
        }
        for name in class_names {
            let class_name = split_name(name).0;
            let Some(node) = self.classes.get_mut(class_name) else {
                continue;
            };
            node.parent = Some(id.to_string());
            self.namespaces[id].classes.insert(class_name.to_string());
        }
        for name in note_names {
            let Some(note) = self.notes.get_mut(name) else {
                continue;
            };
            note.parent = Some(id.to_string());
            self.namespaces[id].notes.insert(name.clone());
        }
    }

    pub fn set_css_style(&mut self, id: &str, styles: &[String]) {
        let Some(node) = self.classes.get_mut(id) else {
            return;
        };
        for s in styles {
            node.styles.extend(s.split(',').map(str::to_string));
        }
    }
}

/// A namespace as `JSON.stringify` writes it: its classes, notes and children
/// in place.
struct NamespaceJson<'m> {
    model: &'m ClassModel,
    node: &'m NamespaceNode,
}

impl Serialize for NamespaceJson<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let model = self.model;
        let node = self.node;
        let mut out = s.serialize_struct("NamespaceNode", 7)?;
        out.serialize_field("id", &node.id)?;
        out.serialize_field("label", &node.label)?;
        out.serialize_field("classes", &Lookup(&node.classes, &model.classes))?;
        out.serialize_field("notes", &Lookup(&node.notes, &model.notes))?;
        out.serialize_field(
            "children",
            &Children {
                model,
                ids: &node.children,
            },
        )?;
        if let Some(parent) = &node.parent {
            out.serialize_field("parent", parent)?;
        }
        out.serialize_field("explicit", &node.explicit)?;
        out.end()
    }
}

/// The entries of `map` that `ids` names, as an object.
struct Lookup<'m, T>(&'m IndexSet<String>, &'m IndexMap<String, T>);

impl<T: Serialize> Serialize for Lookup<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(
            self.0
                .iter()
                .filter_map(|id| self.1.get(id).map(|value| (id, value))),
        )
    }
}

struct Children<'m> {
    model: &'m ClassModel,
    ids: &'m IndexSet<String>,
}

impl Serialize for Children<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out = s.serialize_map(Some(self.ids.len()))?;
        for id in self.ids {
            if let Some(node) = self.model.namespaces.get(id) {
                out.serialize_entry(
                    id,
                    &NamespaceJson {
                        model: self.model,
                        node,
                    },
                )?;
            }
        }
        out.end()
    }
}

struct Namespaces<'m>(&'m ClassModel);

impl Serialize for Namespaces<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(self.0.namespaces.iter().map(|(id, node)| {
            (
                id,
                NamespaceJson {
                    model: self.0,
                    node,
                },
            )
        }))
    }
}

struct Tagged<'m>(&'m ClassModel);

impl Serialize for Tagged<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let classes = &self.0.classes;
        s.collect_map(self.0.with_css_class.iter().map(|(name, ids)| {
            (
                name,
                ids.iter()
                    .filter_map(|id| classes.get(id))
                    .collect::<Vec<_>>(),
            )
        }))
    }
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct LineTypes {
    LINE: LineType,
    DOTTED_LINE: LineType,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct RelationTypes {
    AGGREGATION: u8,
    EXTENSION: u8,
    COMPOSITION: u8,
    DEPENDENCY: u8,
    LOLLIPOP: u8,
}

impl Serialize for ClassModel {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out = s.serialize_struct("ClassDb", 15)?;
        out.serialize_field("type", "class")?;
        out.serialize_field(
            "lineType",
            &LineTypes {
                LINE: LineType::Line,
                DOTTED_LINE: LineType::Dotted,
            },
        )?;
        out.serialize_field(
            "relationType",
            &RelationTypes {
                AGGREGATION: RelationType::Aggregation as u8,
                EXTENSION: RelationType::Extension as u8,
                COMPOSITION: RelationType::Composition as u8,
                DEPENDENCY: RelationType::Dependency as u8,
                LOLLIPOP: RelationType::Lollipop as u8,
            },
        )?;
        out.serialize_field("direction", &self.direction)?;
        out.serialize_field("classes", &self.classes)?;
        out.serialize_field("relations", &self.relations)?;
        out.serialize_field("notes", &self.notes)?;
        out.serialize_field("namespaces", &Namespaces(self))?;
        out.serialize_field("interfaces", &self.interfaces)?;
        out.serialize_field("styleClasses", &self.style_classes)?;
        if let Some(title) = &self.titles.title {
            out.serialize_field("title", title)?;
        }
        if let Some(acc_title) = &self.titles.acc_title {
            out.serialize_field("accTitle", acc_title)?;
        }
        if let Some(acc_descr) = &self.titles.acc_descr {
            out.serialize_field("accDescr", acc_descr)?;
        }
        out.serialize_field("namespaceStack", &self.namespace_stack)?;
        out.serialize_field("withCssClass", &Tagged(self))?;
        out.end()
    }
}
