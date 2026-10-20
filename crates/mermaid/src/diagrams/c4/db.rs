//! Pele's `diagrams/c4/db.ts`.

use crate::diagrams::common::db::Titles;
use indexmap::IndexMap;
use serde::{
    Serialize, Serializer,
    ser::{Error as _, SerializeMap},
};

use super::types::{
    C4Attr, C4Element, C4Named, C4Ref, C4Rel, C4Type, NodeType, RelKind, ShapeBase, ShapeKind,
};
use crate::js::{self, JsonNumber};

/// A field of a shape, a boundary or a relation that an argument may set.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Label,
    Type,
    Descr,
    Techn,
    Sprite,
    Tags,
    Link,
    BgColor,
    FontColor,
    BorderColor,
    Shadowing,
    Shape,
    LegendText,
    LegendSprite,
    TextColor,
    LineColor,
}

impl Field {
    /// The field a named argument's key names.
    fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "label" => Self::Label,
            "type" => Self::Type,
            "descr" => Self::Descr,
            "techn" => Self::Techn,
            "sprite" => Self::Sprite,
            "tags" => Self::Tags,
            "link" => Self::Link,
            "bgColor" => Self::BgColor,
            "fontColor" => Self::FontColor,
            "borderColor" => Self::BorderColor,
            "shadowing" => Self::Shadowing,
            "shape" => Self::Shape,
            "legendText" => Self::LegendText,
            "legendSprite" => Self::LegendSprite,
            "textColor" => Self::TextColor,
            "lineColor" => Self::LineColor,
            _ => return None,
        })
    }
}

/// A shape, a boundary or a relation, as the arguments fill it.
trait Target {
    /// The text field `field`, when this target has it. The label is not one.
    fn slot(&mut self, field: Field) -> Option<&mut Option<String>>;
    fn label_mut(&mut self) -> (&mut String, &mut Option<C4Named>);
    fn extra_mut(&mut self) -> &mut Option<IndexMap<String, String>>;

    /// Mermaid assigns a named argument to whatever property it names,
    /// structural ones included. Here it reaches only the known fields; any
    /// other name is kept aside in `extra`.
    fn named(&mut self, attr: &C4Named) {
        self.named_field(attr);
    }

    /// [`Target::named`] for the fields [`Target::set`] knows.
    fn named_field(&mut self, attr: &C4Named) {
        if let Some(field) = Field::from_key(&attr.key)
            && self.set(field, attr.value.clone())
        {
            return;
        }
        self.extra_mut()
            .get_or_insert_with(IndexMap::new)
            .insert(attr.key.clone(), attr.value.clone());
    }

    /// Sets `field` to `value`; false when this target has no such field.
    fn set(&mut self, field: Field, value: String) -> bool {
        if field == Field::Label {
            *self.label_mut().0 = value;
            return true;
        }
        let Some(slot) = self.slot(field) else {
            return false;
        };
        *slot = Some(value);
        true
    }

    /// A positional argument fills the field of its slot. A named one fills
    /// the field it names, whichever slot it is in.
    fn put(&mut self, field: Field, attr: Option<&C4Attr>) {
        match attr {
            None => {}
            Some(C4Attr::Str(value)) => {
                self.set(field, value.clone());
            }
            Some(C4Attr::Named(attr)) => self.named(attr),
        }
    }

    /// [`Target::put`] for a text field, which is the empty string when its
    /// slot is empty.
    fn text(&mut self, field: Field, attr: Option<&C4Attr>) {
        if attr.is_some() {
            self.put(field, attr);
        } else if let Some(slot) = self.slot(field) {
            slot.get_or_insert_with(String::new);
        }
    }

    fn label(&mut self, attr: Option<&C4Attr>) {
        let (text, label_attr) = self.label_mut();
        *label_attr = None;
        match attr {
            Some(C4Attr::Named(attr)) => {
                *text = String::new();
                *label_attr = Some(attr.clone());
                self.named(attr);
            }
            Some(C4Attr::Str(value)) => *text = value.clone(),
            None => *text = String::new(),
        }
    }
}

impl Target for C4Element {
    fn slot(&mut self, field: Field) -> Option<&mut Option<String>> {
        Some(match field {
            Field::Type => &mut self.ty,
            Field::Descr => &mut self.descr,
            Field::Techn => &mut self.techn,
            Field::Sprite => &mut self.sprite,
            Field::Tags => &mut self.tags,
            Field::Link => &mut self.link,
            Field::BgColor => &mut self.bg_color,
            Field::FontColor => &mut self.font_color,
            Field::BorderColor => &mut self.border_color,
            Field::Shadowing => &mut self.shadowing,
            Field::Shape => &mut self.shape,
            Field::LegendText => &mut self.legend_text,
            Field::LegendSprite => &mut self.legend_sprite,
            Field::Label | Field::TextColor | Field::LineColor => return None,
        })
    }

    fn label_mut(&mut self) -> (&mut String, &mut Option<C4Named>) {
        (&mut self.label, &mut self.label_attr)
    }

    fn extra_mut(&mut self) -> &mut Option<IndexMap<String, String>> {
        &mut self.extra
    }
}

impl Target for C4Rel {
    fn slot(&mut self, field: Field) -> Option<&mut Option<String>> {
        Some(match field {
            Field::Descr => &mut self.descr,
            Field::Techn => &mut self.techn,
            Field::Sprite => &mut self.sprite,
            Field::Tags => &mut self.tags,
            Field::Link => &mut self.link,
            Field::TextColor => &mut self.text_color,
            Field::LineColor => &mut self.line_color,
            _ => return None,
        })
    }

    fn label_mut(&mut self) -> (&mut String, &mut Option<C4Named>) {
        (&mut self.label, &mut self.label_attr)
    }

    fn extra_mut(&mut self) -> &mut Option<IndexMap<String, String>> {
        &mut self.extra
    }

    fn named(&mut self, attr: &C4Named) {
        match attr.key.as_str() {
            "offsetX" => self.offset_x = Some(parse_int(&attr.value)),
            "offsetY" => self.offset_y = Some(parse_int(&attr.value)),
            _ => self.named_field(attr),
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
    list[at].label(title);
    at
}

/// Pele's `C4Db`, serialized with its lookups as Pele's is.
#[derive(Clone, Debug, PartialEq)]
pub struct C4Model {
    pub c4_type: Option<C4Type>,
    pub titles: Titles,
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
            titles: Titles::default(),
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
    /// A person or system takes alias, label, ?descr, ?sprite, ?tags, $link;
    /// a container or component alias, label, ?techn, ?descr, ?sprite,
    /// ?tags, $link.
    pub fn add_shape(&mut self, kind: ShapeKind, attrs: &[C4Attr]) {
        let at = element(
            &mut self.shapes,
            &mut self.shape_index,
            attrs.first(),
            attrs.get(1),
        );
        let current = self.current;
        let shape = &mut self.shapes[at];
        let from = match kind.base {
            ShapeBase::Person | ShapeBase::System => {
                shape.text(Field::Descr, attrs.get(2));
                3
            }
            ShapeBase::Container | ShapeBase::Component => {
                shape.text(Field::Techn, attrs.get(2));
                shape.text(Field::Descr, attrs.get(3));
                4
            }
        };
        shape.put(Field::Sprite, attrs.get(from));
        shape.put(Field::Tags, attrs.get(from + 1));
        shape.put(Field::Link, attrs.get(from + 2));
        shape.kind = Some(kind);
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
            boundary.put(Field::Type, attrs.get(2));
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
        boundary.put(Field::Tags, attrs.get(3));
        boundary.put(Field::Link, attrs.get(4));
        self.open(at);
    }

    /// alias, label, ?type, ?descr, ?sprite, ?tags, $link
    pub fn add_deployment_node(&mut self, node_type: NodeType, attrs: &[C4Attr]) {
        let at = self.boundary(attrs, "node");
        let boundary = &mut self.boundaries[at];
        boundary.text(Field::Descr, attrs.get(3));
        // Mermaid drops whatever is in the sprite slot, a named argument
        // included.
        boundary.put(Field::Sprite, attrs.get(4));
        boundary.put(Field::Tags, attrs.get(5));
        boundary.put(Field::Link, attrs.get(6));
        boundary.node_type = Some(node_type);
        self.open(at);
    }

    pub fn pop_boundary_parse_stack(&mut self) {
        self.current = self.stack.pop().unwrap_or(0);
    }

    /// from, to, label, ?techn, ?descr, ?sprite, ?tags, $link
    pub fn add_rel(&mut self, ty: RelKind, attrs: &[C4Attr]) {
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
                ty,
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
        rel.ty = ty;
        rel.label(attrs.get(2));
        rel.text(Field::Techn, attrs.get(3));
        rel.text(Field::Descr, attrs.get(4));
        rel.put(Field::Sprite, attrs.get(5));
        rel.put(Field::Tags, attrs.get(6));
        rel.put(Field::Link, attrs.get(7));
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
    pub fn update_el_style(&mut self, attrs: &[C4Attr]) {
        let Some(C4Attr::Str(name)) = attrs.first() else {
            return;
        };
        let Some(at) = self.get_c4_shape(name) else {
            return;
        };
        let el = self.element_mut(at);
        for (i, field) in [
            Field::BgColor,
            Field::FontColor,
            Field::BorderColor,
            Field::Shadowing,
            Field::Shape,
            Field::Sprite,
            Field::Techn,
            Field::LegendText,
            Field::LegendSprite,
        ]
        .into_iter()
        .enumerate()
        {
            el.put(field, attrs.get(i + 1));
        }
    }

    /// from, to, ?textColor, ?lineColor, ?offsetX, ?offsetY
    pub fn update_rel_style(&mut self, attrs: &[C4Attr]) {
        let (Some(C4Attr::Str(from)), Some(C4Attr::Str(to))) = (attrs.first(), attrs.get(1)) else {
            return;
        };
        let Some(&at) = self.rel_index.get(from).and_then(|targets| targets.get(to)) else {
            return;
        };
        let rel = &mut self.rels[at];
        rel.put(Field::TextColor, attrs.get(2));
        rel.put(Field::LineColor, attrs.get(3));
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
    pub fn update_layout_config(&mut self, attrs: &[C4Attr]) {
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
        ] {
            if let Some(value) = value {
                map.serialize_entry(key, value)?;
            }
        }
        if let Some(kind) = &el.kind {
            map.serialize_entry("kind", kind)?;
        }
        if let Some(node_type) = &el.node_type {
            map.serialize_entry("nodeType", node_type)?;
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
        if let Some(c4_type) = &self.c4_type {
            map.serialize_entry("c4Type", c4_type)?;
        }
        for (key, value) in [
            ("title", &self.titles.title),
            ("accTitle", &self.titles.acc_title),
            ("accDescr", &self.titles.acc_descr),
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
