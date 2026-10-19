//! Pele's `diagrams/c4/db.ts`.

use indexmap::IndexMap;
use serde::{
    Serialize, Serializer,
    ser::{Error as _, SerializeMap},
};

use super::types::{C4Attr, C4Element, C4Named, C4Ref, C4Rel};
use crate::js::{self, JsonNumber, Lazy};

static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");

const ELEMENT_FIELDS: [&str; 14] = [
    "label",
    "type",
    "descr",
    "techn",
    "sprite",
    "tags",
    "link",
    "bgColor",
    "fontColor",
    "borderColor",
    "shadowing",
    "shape",
    "legendText",
    "legendSprite",
];
const REL_FIELDS: [&str; 8] = [
    "label",
    "descr",
    "techn",
    "sprite",
    "tags",
    "link",
    "textColor",
    "lineColor",
];

/// A shape, a boundary or a relation, as the argument helpers fill it.
trait Target {
    /// The field `name`, one of this target's known ones, set to `value`.
    fn set(&mut self, name: &str, value: String);
    /// The text field `name`, `descr` or `techn`.
    fn text_mut(&mut self, name: &str) -> &mut Option<String>;
    fn label_mut(&mut self) -> (&mut String, &mut Option<C4Named>);
    /// Mermaid assigns a named argument to whatever property it names,
    /// structural ones included. Here it reaches only the known fields; any
    /// other name is kept aside in `extra`.
    fn named(&mut self, attr: &C4Named);
}

impl Target for C4Element {
    fn set(&mut self, name: &str, value: String) {
        let field = match name {
            "label" => {
                self.label = value;
                return;
            }
            "type" => &mut self.ty,
            "descr" => &mut self.descr,
            "techn" => &mut self.techn,
            "sprite" => &mut self.sprite,
            "tags" => &mut self.tags,
            "link" => &mut self.link,
            "bgColor" => &mut self.bg_color,
            "fontColor" => &mut self.font_color,
            "borderColor" => &mut self.border_color,
            "shadowing" => &mut self.shadowing,
            "shape" => &mut self.shape,
            "legendText" => &mut self.legend_text,
            "legendSprite" => &mut self.legend_sprite,
            _ => unreachable!("not an element field: {name}"),
        };
        *field = Some(value);
    }

    fn text_mut(&mut self, name: &str) -> &mut Option<String> {
        if name == "descr" {
            &mut self.descr
        } else {
            &mut self.techn
        }
    }

    fn label_mut(&mut self) -> (&mut String, &mut Option<C4Named>) {
        (&mut self.label, &mut self.label_attr)
    }

    fn named(&mut self, attr: &C4Named) {
        if ELEMENT_FIELDS.contains(&attr.key.as_str()) {
            self.set(&attr.key, attr.value.clone());
        } else {
            self.extra
                .get_or_insert_with(IndexMap::new)
                .insert(attr.key.clone(), attr.value.clone());
        }
    }
}

impl Target for C4Rel {
    fn set(&mut self, name: &str, value: String) {
        let field = match name {
            "label" => {
                self.label = value;
                return;
            }
            "descr" => &mut self.descr,
            "techn" => &mut self.techn,
            "sprite" => &mut self.sprite,
            "tags" => &mut self.tags,
            "link" => &mut self.link,
            "textColor" => &mut self.text_color,
            "lineColor" => &mut self.line_color,
            _ => unreachable!("not a relation field: {name}"),
        };
        *field = Some(value);
    }

    fn text_mut(&mut self, name: &str) -> &mut Option<String> {
        if name == "descr" {
            &mut self.descr
        } else {
            &mut self.techn
        }
    }

    fn label_mut(&mut self) -> (&mut String, &mut Option<C4Named>) {
        (&mut self.label, &mut self.label_attr)
    }

    fn named(&mut self, attr: &C4Named) {
        let key = attr.key.as_str();
        if key == "offsetX" {
            self.offset_x = Some(parse_int(&attr.value));
        } else if key == "offsetY" {
            self.offset_y = Some(parse_int(&attr.value));
        } else if REL_FIELDS.contains(&key) {
            self.set(key, attr.value.clone());
        } else {
            self.extra
                .get_or_insert_with(IndexMap::new)
                .insert(attr.key.clone(), attr.value.clone());
        }
    }
}

/// `parseInt(text)`: hexadecimal after `0x`, decimal otherwise.
fn parse_int(text: &str) -> f64 {
    let rest = js::trim_start(text);
    let rest = rest
        .strip_prefix('-')
        .or_else(|| rest.strip_prefix('+'))
        .unwrap_or(rest);
    if rest.starts_with("0x") || rest.starts_with("0X") {
        js::parse_int(text, 16)
    } else {
        js::parse_int(text, 10)
    }
}

/// A positional argument fills the field of its slot. A named one fills the
/// field it names, whichever slot it is in.
fn put(target: &mut impl Target, field: &str, attr: Option<&C4Attr>) {
    match attr {
        None => {}
        Some(C4Attr::Str(value)) => target.set(field, value.clone()),
        Some(C4Attr::Named(attr)) => target.named(attr),
    }
}

fn text(target: &mut impl Target, field: &str, attr: Option<&C4Attr>) {
    if attr.is_none() {
        target.text_mut(field).get_or_insert_with(String::new);
    } else {
        put(target, field, attr);
    }
}

fn label(target: &mut impl Target, attr: Option<&C4Attr>) {
    let (text, label_attr) = target.label_mut();
    *label_attr = None;
    match attr {
        Some(C4Attr::Named(attr)) => {
            *text = String::new();
            *label_attr = Some(attr.clone());
            target.named(attr);
        }
        Some(C4Attr::Str(value)) => *text = value.clone(),
        None => *text = String::new(),
    }
}

fn element(
    list: &mut Vec<C4Element>,
    index: &mut IndexMap<String, usize>,
    alias: Option<&C4Attr>,
    title: Option<&C4Attr>,
) -> usize {
    let found = match alias {
        Some(C4Attr::Str(alias)) => index.get(alias).copied(),
        _ => None,
    };
    let at = found.unwrap_or_else(|| {
        let at = list.len();
        let mut el = C4Element::default();
        match alias {
            Some(C4Attr::Str(alias)) => {
                el.alias = alias.clone();
                index.insert(alias.clone(), at);
            }
            Some(C4Attr::Named(attr)) => {
                el.alias_attr = Some(attr.clone());
                el.named(attr);
            }
            None => {}
        }
        list.push(el);
        at
    });
    label(&mut list[at], title);
    at
}

/// Pele's `C4Db`, serialized with its lookups as Pele's is.
#[derive(Clone, Debug, PartialEq)]
pub struct C4Model {
    /// The header keyword: C4Context, C4Container, C4Component, C4Dynamic or
    /// C4Deployment.
    pub c4_type: Option<String>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub shapes: Vec<C4Element>,
    /// The first boundary is the unnamed one that holds everything at the top
    /// level.
    pub boundaries: Vec<C4Element>,
    pub rels: Vec<C4Rel>,
    pub shape_in_row: f64,
    pub boundary_in_row: f64,
    shape_index: IndexMap<String, usize>,
    /// The top-level boundary is not listed, so a boundary a diagram names
    /// `global` is its own box. In Mermaid it replaces the top level and
    /// nothing is drawn.
    boundary_index: IndexMap<String, usize>,
    rel_index: IndexMap<String, IndexMap<String, usize>>,
    current: usize,
    stack: Vec<usize>,
}

impl Default for C4Model {
    fn default() -> Self {
        Self {
            c4_type: None,
            title: None,
            acc_title: None,
            acc_descr: None,
            shapes: Vec::new(),
            boundaries: vec![C4Element {
                alias: "global".into(),
                label: "global".into(),
                ty: Some("global".into()),
                ..C4Element::default()
            }],
            rels: Vec::new(),
            shape_in_row: 4.0,
            boundary_in_row: 2.0,
            shape_index: IndexMap::new(),
            boundary_index: IndexMap::new(),
            rel_index: IndexMap::new(),
            current: 0,
            stack: Vec::new(),
        }
    }
}

impl C4Model {
    pub fn set_c4_type(&mut self, ty: &str) {
        self.c4_type = Some(ty.to_string());
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = Some(title.to_string());
    }

    pub fn set_acc_description(&mut self, descr: &str) {
        self.acc_descr = Some(RE_INDENT.replace(descr, |_| "\n".into()));
    }

    /// alias, label, ?descr, ?sprite, ?tags, $link
    pub fn add_person_or_system(&mut self, kind: &str, attrs: &[C4Attr]) {
        let at = element(
            &mut self.shapes,
            &mut self.shape_index,
            attrs.first(),
            attrs.get(1),
        );
        text(&mut self.shapes[at], "descr", attrs.get(2));
        self.finish_shape(at, kind, attrs, 3);
    }

    /// alias, label, ?techn, ?descr, ?sprite, ?tags, $link
    pub fn add_container(&mut self, kind: &str, attrs: &[C4Attr]) {
        let at = element(
            &mut self.shapes,
            &mut self.shape_index,
            attrs.first(),
            attrs.get(1),
        );
        text(&mut self.shapes[at], "techn", attrs.get(2));
        text(&mut self.shapes[at], "descr", attrs.get(3));
        self.finish_shape(at, kind, attrs, 4);
    }

    pub fn add_component(&mut self, kind: &str, attrs: &[C4Attr]) {
        self.add_container(kind, attrs);
    }

    fn finish_shape(&mut self, at: usize, kind: &str, attrs: &[C4Attr], from: usize) {
        let current = self.current;
        let shape = &mut self.shapes[at];
        put(shape, "sprite", attrs.get(from));
        put(shape, "tags", attrs.get(from + 1));
        put(shape, "link", attrs.get(from + 2));
        shape.kind = Some(kind.to_string());
        shape.parent = Some(current);
    }

    fn boundary(&mut self, attrs: &[C4Attr], fallback: &str) -> usize {
        let at = element(
            &mut self.boundaries,
            &mut self.boundary_index,
            attrs.first(),
            attrs.get(1),
        );
        let boundary = &mut self.boundaries[at];
        if attrs.get(2).is_none() {
            boundary.ty = Some(fallback.to_string());
        } else {
            put(boundary, "type", attrs.get(2));
        }
        at
    }

    fn open(&mut self, at: usize) {
        self.boundaries[at].parent = Some(self.current);
        self.stack.push(self.current);
        self.current = at;
    }

    /// alias, label, ?type, ?tags, $link
    pub fn add_person_or_system_boundary(&mut self, attrs: &[C4Attr]) {
        self.add_boundary(attrs, "system");
    }

    pub fn add_container_boundary(&mut self, attrs: &[C4Attr]) {
        self.add_boundary(attrs, "container");
    }

    fn add_boundary(&mut self, attrs: &[C4Attr], fallback: &str) {
        let at = self.boundary(attrs, fallback);
        let boundary = &mut self.boundaries[at];
        put(boundary, "tags", attrs.get(3));
        put(boundary, "link", attrs.get(4));
        self.open(at);
    }

    /// alias, label, ?type, ?descr, ?sprite, ?tags, $link
    pub fn add_deployment_node(&mut self, node_type: &str, attrs: &[C4Attr]) {
        let at = self.boundary(attrs, "node");
        let boundary = &mut self.boundaries[at];
        text(boundary, "descr", attrs.get(3));
        // Mermaid drops whatever is in the sprite slot, a named argument
        // included.
        put(boundary, "sprite", attrs.get(4));
        put(boundary, "tags", attrs.get(5));
        put(boundary, "link", attrs.get(6));
        boundary.node_type = Some(node_type.to_string());
        self.open(at);
    }

    pub fn pop_boundary_parse_stack(&mut self) {
        self.current = self.stack.pop().unwrap_or(0);
    }

    /// from, to, label, ?techn, ?descr, ?sprite, ?tags, $link
    pub fn add_rel(&mut self, ty: &str, attrs: &[C4Attr]) {
        let (Some(from), Some(to), Some(_)) = (attrs.first(), attrs.get(1), attrs.get(2)) else {
            return;
        };
        let known = match (from, to) {
            (C4Attr::Str(from), C4Attr::Str(to)) => Some((from, to)),
            _ => None,
        };
        let found = known.and_then(|(from, to)| {
            self.rel_index
                .get(from)
                .and_then(|targets| targets.get(to))
                .copied()
        });
        let at = found.unwrap_or_else(|| {
            let at = self.rels.len();
            let mut rel = C4Rel {
                ty: ty.to_string(),
                ..C4Rel::default()
            };
            if let Some((from, to)) = known {
                self.rel_index
                    .entry(from.clone())
                    .or_default()
                    .insert(to.clone(), at);
            }
            match from {
                C4Attr::Str(from) => rel.from = from.clone(),
                C4Attr::Named(attr) => rel.from_attr = Some(attr.clone()),
            }
            match to {
                C4Attr::Str(to) => rel.to = to.clone(),
                C4Attr::Named(attr) => rel.to_attr = Some(attr.clone()),
            }
            self.rels.push(rel);
            at
        });
        let rel = &mut self.rels[at];
        rel.ty = ty.to_string();
        label(rel, attrs.get(2));
        text(rel, "techn", attrs.get(3));
        text(rel, "descr", attrs.get(4));
        put(rel, "sprite", attrs.get(5));
        put(rel, "tags", attrs.get(6));
        put(rel, "link", attrs.get(7));
    }

    pub fn get_c4_shape(&self, alias: &str) -> Option<C4Ref> {
        self.shape_index
            .get(alias)
            .map(|&at| C4Ref::Shape(at))
            .or_else(|| {
                self.boundary_index
                    .get(alias)
                    .map(|&at| C4Ref::Boundary(at))
            })
    }

    pub fn element(&self, at: C4Ref) -> &C4Element {
        match at {
            C4Ref::Shape(at) => &self.shapes[at],
            C4Ref::Boundary(at) => &self.boundaries[at],
        }
    }

    fn element_mut(&mut self, at: C4Ref) -> &mut C4Element {
        match at {
            C4Ref::Shape(at) => &mut self.shapes[at],
            C4Ref::Boundary(at) => &mut self.boundaries[at],
        }
    }

    /// elementName, ?bgColor, ?fontColor, ?borderColor, ?shadowing, ?shape,
    /// ?sprite, ?techn, ?legendText, ?legendSprite
    pub fn update_el_style(&mut self, _ty: &str, attrs: &[C4Attr]) {
        let Some(C4Attr::Str(name)) = attrs.first() else {
            return;
        };
        let Some(at) = self.get_c4_shape(name) else {
            return;
        };
        let el = self.element_mut(at);
        for (i, field) in [
            "bgColor",
            "fontColor",
            "borderColor",
            "shadowing",
            "shape",
            "sprite",
            "techn",
            "legendText",
            "legendSprite",
        ]
        .into_iter()
        .enumerate()
        {
            put(el, field, attrs.get(i + 1));
        }
    }

    /// from, to, ?textColor, ?lineColor, ?offsetX, ?offsetY
    pub fn update_rel_style(&mut self, _ty: &str, attrs: &[C4Attr]) {
        let (Some(C4Attr::Str(from)), Some(C4Attr::Str(to))) = (attrs.first(), attrs.get(1)) else {
            return;
        };
        let Some(&at) = self.rel_index.get(from).and_then(|targets| targets.get(to)) else {
            return;
        };
        let rel = &mut self.rels[at];
        put(rel, "textColor", attrs.get(2));
        put(rel, "lineColor", attrs.get(3));
        match attrs.get(4) {
            Some(C4Attr::Str(x)) => rel.offset_x = Some(parse_int(x)),
            Some(C4Attr::Named(x)) => rel.named(x),
            None => {}
        }
        match attrs.get(5) {
            Some(C4Attr::Str(y)) => rel.offset_y = Some(parse_int(y)),
            Some(C4Attr::Named(y)) => rel.named(y),
            None => {}
        }
    }

    /// ?c4ShapeInRow, ?c4BoundaryInRow. As in Mermaid, the slot decides which
    /// one a value sets, not its name.
    pub fn update_layout_config(&mut self, _ty: &str, attrs: &[C4Attr]) {
        let count = |attr: Option<&C4Attr>| -> f64 {
            match attr {
                None => f64::NAN,
                Some(C4Attr::Str(value)) => parse_int(value),
                Some(C4Attr::Named(attr)) => parse_int(&attr.value),
            }
        };
        let shapes = count(attrs.first());
        let boundaries = count(attrs.get(1));
        if shapes >= 1.0 {
            self.shape_in_row = shapes;
        }
        if boundaries >= 1.0 {
            self.boundary_in_row = boundaries;
        }
    }
}

/// An element as `JSON.stringify` writes it: its parent boundary in full.
struct ElementJson<'a> {
    model: &'a C4Model,
    el: &'a C4Element,
    depth: usize,
}

impl Serialize for ElementJson<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.depth > self.model.boundaries.len() {
            return Err(S::Error::custom("Converting circular structure to JSON"));
        }
        let el = self.el;
        let mut map = s.serialize_map(None)?;
        map.serialize_entry("alias", &el.alias)?;
        map.serialize_entry("label", &el.label)?;
        if let Some(attr) = &el.alias_attr {
            map.serialize_entry("aliasAttr", attr)?;
        }
        if let Some(attr) = &el.label_attr {
            map.serialize_entry("labelAttr", attr)?;
        }
        if let Some(parent) = el.parent {
            map.serialize_entry("parent", &self.boundary(parent))?;
        }
        for (key, value) in [
            ("type", &el.ty),
            ("descr", &el.descr),
            ("techn", &el.techn),
            ("sprite", &el.sprite),
            ("tags", &el.tags),
            ("link", &el.link),
            ("bgColor", &el.bg_color),
            ("fontColor", &el.font_color),
            ("borderColor", &el.border_color),
            ("shadowing", &el.shadowing),
            ("shape", &el.shape),
            ("legendText", &el.legend_text),
            ("legendSprite", &el.legend_sprite),
            ("kind", &el.kind),
            ("nodeType", &el.node_type),
        ] {
            if let Some(value) = value {
                map.serialize_entry(key, value)?;
            }
        }
        if let Some(extra) = &el.extra {
            map.serialize_entry("extra", extra)?;
        }
        map.end()
    }
}

impl<'a> ElementJson<'a> {
    fn boundary(&self, at: usize) -> ElementJson<'a> {
        ElementJson {
            model: self.model,
            el: &self.model.boundaries[at],
            depth: self.depth + 1,
        }
    }
}

impl C4Model {
    fn json<'a>(&'a self, el: &'a C4Element) -> ElementJson<'a> {
        ElementJson {
            model: self,
            el,
            depth: 0,
        }
    }

    fn list_json<'a>(&'a self, list: &'a [C4Element]) -> Vec<ElementJson<'a>> {
        list.iter().map(|el| self.json(el)).collect()
    }

    fn index_json<'a>(
        &'a self,
        index: &'a IndexMap<String, usize>,
        list: &'a [C4Element],
    ) -> IndexMap<&'a String, ElementJson<'a>> {
        index
            .iter()
            .map(|(alias, &at)| (alias, self.json(&list[at])))
            .collect()
    }
}

impl Serialize for C4Model {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        map.serialize_entry("type", "c4")?;
        for (key, value) in [
            ("c4Type", &self.c4_type),
            ("title", &self.title),
            ("accTitle", &self.acc_title),
            ("accDescr", &self.acc_descr),
        ] {
            if let Some(value) = value {
                map.serialize_entry(key, value)?;
            }
        }
        map.serialize_entry("shapes", &self.list_json(&self.shapes))?;
        map.serialize_entry("boundaries", &self.list_json(&self.boundaries))?;
        map.serialize_entry("rels", &self.rels)?;
        map.serialize_entry("shapeInRow", &JsonNumber(self.shape_in_row))?;
        map.serialize_entry("boundaryInRow", &JsonNumber(self.boundary_in_row))?;
        map.serialize_entry(
            "shapeIndex",
            &self.index_json(&self.shape_index, &self.shapes),
        )?;
        map.serialize_entry(
            "boundaryIndex",
            &self.index_json(&self.boundary_index, &self.boundaries),
        )?;
        let rels: IndexMap<&String, IndexMap<&String, &C4Rel>> = self
            .rel_index
            .iter()
            .map(|(from, targets)| {
                (
                    from,
                    targets
                        .iter()
                        .map(|(to, &at)| (to, &self.rels[at]))
                        .collect(),
                )
            })
            .collect();
        map.serialize_entry("relIndex", &rels)?;
        map.serialize_entry("current", &self.json(&self.boundaries[self.current]))?;
        let stack: Vec<ElementJson> = self
            .stack
            .iter()
            .map(|&at| self.json(&self.boundaries[at]))
            .collect();
        map.serialize_entry("stack", &stack)?;
        map.end()
    }
}
