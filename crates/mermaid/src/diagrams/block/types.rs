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

use crate::js::{self, JsonNumber};
use crate::svg::{edges::Marker, shapes::Shape};

/// A statement with links is a list: block, edge, block, and so on.
#[derive(Clone, Debug, PartialEq)]
pub enum Statement {
    One(usize),
    List(Vec<usize>),
}

/// What a block is, Mermaid's `type`: a shape, a container, an edge, or a
/// statement that sets columns, classes, or styles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BlockKind {
    #[default]
    Na,
    Square,
    Round,
    Circle,
    DoubleCircle,
    Diamond,
    Hexagon,
    Stadium,
    Subroutine,
    Cylinder,
    RectLeftInvArrow,
    LeanRight,
    LeanLeft,
    Trapezoid,
    InvTrapezoid,
    BlockArrow,
    Composite,
    Space,
    Edge,
    ColumnSetting,
    ClassDef,
    ApplyClass,
    ApplyStyles,
}

impl BlockKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Na => "na",
            Self::Square => "square",
            Self::Round => "round",
            Self::Circle => "circle",
            Self::DoubleCircle => "doublecircle",
            Self::Diamond => "diamond",
            Self::Hexagon => "hexagon",
            Self::Stadium => "stadium",
            Self::Subroutine => "subroutine",
            Self::Cylinder => "cylinder",
            Self::RectLeftInvArrow => "rect_left_inv_arrow",
            Self::LeanRight => "lean_right",
            Self::LeanLeft => "lean_left",
            Self::Trapezoid => "trapezoid",
            Self::InvTrapezoid => "inv_trapezoid",
            Self::BlockArrow => "block_arrow",
            Self::Composite => "composite",
            Self::Space => "space",
            Self::Edge => "edge",
            Self::ColumnSetting => "column-setting",
            Self::ClassDef => "classDef",
            Self::ApplyClass => "applyClass",
            Self::ApplyStyles => "applyStyles",
        }
    }

    /// The kind of a node written between `brackets`, its opening and
    /// closing ones joined; [`BlockKind::Na`] for a bare id.
    pub fn from_brackets(brackets: Option<&str>) -> Self {
        match brackets {
            Some("[]") => Self::Square,
            Some("()") => Self::Round,
            Some("(())") => Self::Circle,
            Some(">]") => Self::RectLeftInvArrow,
            Some("{}") => Self::Diamond,
            Some("{{}}") => Self::Hexagon,
            Some("([])") => Self::Stadium,
            Some("[[]]") => Self::Subroutine,
            Some("[()]") => Self::Cylinder,
            Some("((()))") => Self::DoubleCircle,
            Some("[//]") => Self::LeanRight,
            Some(r"[\\]") => Self::LeanLeft,
            Some(r"[/\]") => Self::Trapezoid,
            Some(r"[\/]") => Self::InvTrapezoid,
            Some("<[]>") => Self::BlockArrow,
            _ => Self::Na,
        }
    }

    /// The shape a plain block of this kind is drawn as.
    pub fn shape(self) -> Shape {
        match self {
            Self::Round => Shape::Rounded,
            Self::Circle => Shape::Circle,
            Self::DoubleCircle => Shape::DoubleCircle,
            Self::Diamond => Shape::Diamond,
            Self::Hexagon => Shape::Hexagon,
            Self::Stadium => Shape::Stadium,
            Self::Subroutine => Shape::FramedRect,
            Self::Cylinder => Shape::Cylinder,
            Self::RectLeftInvArrow => Shape::Odd,
            Self::LeanRight => Shape::LeanRight,
            Self::LeanLeft => Shape::LeanLeft,
            Self::Trapezoid => Shape::TrapezoidBottom,
            Self::InvTrapezoid => Shape::TrapezoidTop,
            _ => Shape::Rect,
        }
    }
}

/// How many columns a composite's grid has.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Columns {
    /// Every child on one row; `-1` in the JSON.
    #[default]
    Auto,
    Count(f64),
}

impl Columns {
    /// The value Mermaid keeps: `-1` for [`Columns::Auto`].
    pub fn value(self) -> f64 {
        match self {
            Self::Auto => -1.0,
            Self::Count(count) => count,
        }
    }
}

/// The head at one end of an edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrowType {
    Open,
    Point,
    Circle,
    Cross,
}

impl ArrowType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Open => "arrow_open",
            Self::Point => "arrow_point",
            Self::Circle => "arrow_circle",
            Self::Cross => "arrow_cross",
        }
    }

    /// The head at the start of `link`.
    pub fn at_start(link: &str) -> Self {
        match js::trim(link).chars().next() {
            Some('x') => Self::Cross,
            Some('o') => Self::Circle,
            Some('<') => Self::Point,
            _ => Self::Open,
        }
    }

    /// The head at the end of `link`; `None` for an end without one.
    pub fn at_end(link: &str) -> Option<Self> {
        match js::trim(link).chars().next_back() {
            Some('x') => Some(Self::Cross),
            Some('o') => Some(Self::Circle),
            Some('>') => Some(Self::Point),
            _ => None,
        }
    }

    pub fn marker(self) -> Option<Marker> {
        match self {
            Self::Open => None,
            Self::Point => Some(Marker::Arrow),
            Self::Circle => Some(Marker::Circle),
            Self::Cross => Some(Marker::Cross),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Thickness {
    Normal,
    Thick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Solid,
    Dotted,
}

/// How an edge is drawn, read from its link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeLine {
    pub arrow_start: ArrowType,
    /// `""` in the JSON when `None`.
    pub arrow_end: Option<ArrowType>,
    pub thickness: Thickness,
    pub pattern: Pattern,
}

impl EdgeLine {
    pub fn of(link: &str) -> Self {
        Self {
            arrow_start: ArrowType::at_start(link),
            arrow_end: ArrowType::at_end(link),
            thickness: if link.contains("==") {
                Thickness::Thick
            } else {
                Thickness::Normal
            },
            pattern: if link.contains(".-") {
                Pattern::Dotted
            } else {
                Pattern::Solid
            },
        }
    }
}

/// One shape for blocks, edges, and the statements that set columns,
/// classes, and styles, as in Mermaid.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Block {
    pub id: Option<String>,
    pub kind: BlockKind,
    pub label: Option<String>,
    pub children: Option<Vec<usize>>,
    pub columns: Option<Columns>,
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
    /// Set on an edge.
    pub line: Option<EdgeLine>,
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
        if let Some(id) = &block.id {
            map.serialize_entry("id", id)?;
        }
        map.serialize_entry("type", block.kind.name())?;
        let strings = [
            ("label", &block.label),
            ("start", &block.start),
            ("end", &block.end),
        ];
        for (key, value) in strings {
            if let Some(value) = value {
                map.serialize_entry(key, value)?;
            }
        }
        if let Some(line) = &block.line {
            map.serialize_entry("arrowTypeStart", line.arrow_start.name())?;
            map.serialize_entry("arrowTypeEnd", line.arrow_end.map_or("", ArrowType::name))?;
            map.serialize_entry(
                "thickness",
                match line.thickness {
                    Thickness::Normal => "normal",
                    Thickness::Thick => "thick",
                },
            )?;
            map.serialize_entry(
                "pattern",
                match line.pattern {
                    Pattern::Solid => "solid",
                    Pattern::Dotted => "dotted",
                },
            )?;
        }
        let strings = [
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
            ("columns", block.columns.map(Columns::value)),
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
