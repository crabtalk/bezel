//! Pele's `diagrams/block/types.ts`. Pele's model is a graph of shared
//! objects: a block sits in its parent's children and in `byId` at once, and
//! a later statement changes it in both. Here every block is an entry of
//! [`BlockModel::nodes`] and is named by its index, and the class lists that
//! Pele shares between the copies of a space block are entries of
//! [`BlockModel::class_lists`].

use crate::diagrams::common::style_class::StyleClass;
use indexmap::{IndexMap, IndexSet};
use serde::{
    Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq},
};

use crate::js::JsonNumber;

/// A statement with links is a list: block, edge, block, and so on.
#[derive(Clone, Debug, PartialEq)]
pub enum Statement {
    One(usize),
    List(Vec<usize>),
}

/// One shape for blocks, edges, and the statements that set columns,
/// classes, and styles, as in Mermaid.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Block {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub label: Option<String>,
    pub children: Option<Vec<usize>>,
    pub columns: Option<f64>,
    pub width_in_columns: Option<f64>,
    /// Number of cells a `space` statement asked for.
    pub width: Option<f64>,
    pub directions: Option<Vec<String>>,
    /// An entry of [`BlockModel::class_lists`].
    pub classes: Option<usize>,
    pub styles: Option<Vec<String>>,
    pub color_index: Option<f64>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub arrow_type_start: Option<String>,
    pub arrow_type_end: Option<String>,
    pub thickness: Option<String>,
    pub pattern: Option<String>,
    pub css: Option<String>,
    pub style_class: Option<String>,
    pub styles_str: Option<String>,
    /// The statements of a composite before the hierarchy is built, which
    /// Pele keeps in `children`.
    pub statements: Vec<Statement>,
    /// A composite whose header was a link statement holds that statement's
    /// blocks and edges under the keys `0`, `1`, …, as spreading an array
    /// into an object does.
    pub spread: Vec<usize>,
}

/// Pele's `BlockDb`, which is its model, with its private fields: they are
/// in the JSON Pele writes for it.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockModel {
    pub title: Option<String>,
    pub root: usize,
    pub edges: Vec<usize>,
    pub classes: IndexMap<String, StyleClass>,
    /// Mermaid keeps the root in here too, which makes a block called `root`
    /// vanish. Here it stays out.
    pub by_id: IndexMap<String, usize>,
    /// Ids that a class or style statement named before any block had them.
    pub pending: IndexSet<String>,
    pub edge_count: IndexMap<String, usize>,
    pub ids: usize,
    pub colors: usize,
    pub spaces: f64,
    pub nodes: Vec<Block>,
    pub class_lists: Vec<Vec<String>>,
}

impl BlockModel {
    pub fn block(&self, at: usize) -> &Block {
        &self.nodes[at]
    }

    /// The classes of a block: none when it has no list.
    pub fn classes_of(&self, block: &Block) -> &[String] {
        block.classes.map_or(&[], |at| &self.class_lists[at])
    }

    /// `this.blocks`: the root's children.
    pub fn blocks(&self) -> &[usize] {
        self.nodes[self.root].children.as_deref().unwrap_or(&[])
    }
}

struct Node<'a>(&'a BlockModel, usize);

struct Nodes<'a>(&'a BlockModel, &'a [usize]);

impl Serialize for Nodes<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.1.len()))?;
        for &at in self.1 {
            seq.serialize_element(&Node(self.0, at))?;
        }
        seq.end()
    }
}

impl Serialize for Node<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let model = self.0;
        let block = model.block(self.1);
        let mut map = s.serialize_map(None)?;
        for (i, &at) in block.spread.iter().enumerate() {
            map.serialize_entry(&i.to_string(), &Node(model, at))?;
        }
        let strings = [
            ("id", &block.id),
            ("type", &block.kind),
            ("label", &block.label),
            ("start", &block.start),
            ("end", &block.end),
            ("arrowTypeStart", &block.arrow_type_start),
            ("arrowTypeEnd", &block.arrow_type_end),
            ("thickness", &block.thickness),
            ("pattern", &block.pattern),
            ("css", &block.css),
            ("styleClass", &block.style_class),
            ("stylesStr", &block.styles_str),
        ];
        for (key, value) in strings {
            if let Some(value) = value {
                map.serialize_entry(key, value)?;
            }
        }
        let numbers = [
            ("columns", block.columns),
            ("widthInColumns", block.width_in_columns),
            ("width", block.width),
            ("colorIndex", block.color_index),
        ];
        for (key, value) in numbers {
            if let Some(value) = value {
                map.serialize_entry(key, &JsonNumber(value))?;
            }
        }
        if let Some(directions) = &block.directions {
            map.serialize_entry("directions", directions)?;
        }
        if let Some(classes) = block.classes {
            map.serialize_entry("classes", &model.class_lists[classes])?;
        }
        if let Some(styles) = &block.styles {
            map.serialize_entry("styles", styles)?;
        }
        if let Some(children) = &block.children {
            map.serialize_entry("children", &Nodes(model, children))?;
        }
        map.end()
    }
}

struct ById<'a>(&'a BlockModel);

impl Serialize for ById<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.by_id.len()))?;
        for (id, &at) in &self.0.by_id {
            map.serialize_entry(id, &Node(self.0, at))?;
        }
        map.end()
    }
}

impl Serialize for BlockModel {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        map.serialize_entry("type", "block")?;
        if let Some(title) = &self.title {
            map.serialize_entry("title", title)?;
        }
        map.serialize_entry("root", &Node(self, self.root))?;
        map.serialize_entry("blocks", &Nodes(self, self.blocks()))?;
        map.serialize_entry("edges", &Nodes(self, &self.edges))?;
        map.serialize_entry("classes", &self.classes)?;
        map.serialize_entry("byId", &ById(self))?;
        map.serialize_entry("pending", &self.pending)?;
        map.serialize_entry("edgeCount", &self.edge_count)?;
        map.serialize_entry("ids", &self.ids)?;
        map.serialize_entry("colors", &self.colors)?;
        map.serialize_entry("spaces", &JsonNumber(self.spaces))?;
        map.end()
    }
}
