//! Shape names: Mermaid's documented ones and their aliases. Pele's
//! `diagrams/flowchart/shapes.ts`.

use std::{collections::HashMap, sync::LazyLock};

use crate::{geom::Size, svg::shapes::Shape};

/// Mermaid's documented shape names: the short name first, then its aliases.
const SHAPES: [&[&str]; 53] = [
    &["rect", "proc", "process", "rectangle", "squareRect"],
    &["rounded", "event", "roundedRect"],
    &["stadium", "terminal", "pill"],
    &[
        "fr-rect",
        "subprocess",
        "subproc",
        "framed-rectangle",
        "subroutine",
    ],
    &["cyl", "db", "database", "cylinder"],
    &["datastore", "data-store"],
    &["folder", "directory"],
    &["bucket"],
    &["console"],
    &["browser"],
    &["person"],
    &["circle", "circ"],
    &["bang"],
    &["cloud"],
    &["diam", "decision", "diamond", "question"],
    &["hex", "hexagon", "prepare"],
    &["lean-r", "lean-right", "in-out", "lean_right"],
    &["lean-l", "lean-left", "out-in", "lean_left"],
    &["trap-b", "priority", "trapezoid-bottom", "trapezoid"],
    &[
        "trap-t",
        "manual",
        "trapezoid-top",
        "inv-trapezoid",
        "inv_trapezoid",
    ],
    &["dbl-circ", "double-circle", "doublecircle"],
    &["text"],
    &["notch-rect", "card", "notched-rectangle"],
    &[
        "lin-rect",
        "lined-rectangle",
        "lined-process",
        "lin-proc",
        "shaded-process",
    ],
    &["sm-circ", "start", "small-circle", "stateStart"],
    &["fr-circ", "stop", "framed-circle", "stateEnd"],
    &["fork", "join", "forkJoin"],
    &["hourglass", "collate"],
    &["brace", "comment", "brace-l"],
    &["brace-r"],
    &["braces"],
    &["bolt", "com-link", "lightning-bolt"],
    &["doc", "document"],
    &["delay", "half-rounded-rectangle"],
    &["h-cyl", "das", "horizontal-cylinder"],
    &["lin-cyl", "disk", "lined-cylinder"],
    &["curv-trap", "curved-trapezoid", "display"],
    &[
        "div-rect",
        "div-proc",
        "divided-rectangle",
        "divided-process",
    ],
    &["tri", "extract", "triangle"],
    &["win-pane", "internal-storage", "window-pane"],
    &["f-circ", "junction", "filled-circle"],
    &["notch-pent", "loop-limit", "notched-pentagon"],
    &["flip-tri", "manual-file", "flipped-triangle"],
    &["sl-rect", "manual-input", "sloped-rectangle"],
    &["docs", "documents", "st-doc", "stacked-document"],
    &["st-rect", "procs", "processes", "stacked-rectangle"],
    &["bow-rect", "stored-data", "bow-tie-rectangle"],
    &["cross-circ", "summary", "crossed-circle"],
    &["tag-doc", "tagged-document"],
    &["tag-rect", "tagged-rectangle", "tag-proc", "tagged-process"],
    &["flag", "paper-tape"],
    &["odd", "rect_left_inv_arrow"],
    &["lin-doc", "lined-document"],
];

/// Shapes Mermaid accepts by name without documenting them.
const INTERNAL: [&str; 27] = [
    "state",
    "choice",
    "note",
    "composite",
    "rectWithTitle",
    "labelRect",
    "block_arrow",
    "collapsedGroup",
    "iconSquare",
    "iconCircle",
    "icon",
    "iconRounded",
    "imageSquare",
    "anchor",
    "kanbanItem",
    "mindmapCircle",
    "defaultMindmapNode",
    "classBox",
    "erBox",
    "requirementBox",
    "usecaseActor",
    "usecaseActorHollow",
    "usecaseActorAwesome",
    "usecaseActorIcon",
    "usecaseBusiness",
    "usecaseEllipse",
    "usecaseJsonTable",
];
/// Shapes produced by the bracket syntax, mapped to their documented names.
const SYNTAX: [(&str, &str); 5] = [
    ("square", "rect"),
    ("round", "rounded"),
    ("ellipse", "ellipse"),
    ("hexagon", "hex"),
    ("diamond", "diam"),
];

static CANONICAL: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut canonical = HashMap::new();
    for names in SHAPES {
        for name in names {
            canonical.insert(*name, names[0]);
        }
    }
    for name in INTERNAL {
        canonical.insert(name, name);
    }
    canonical
});

pub fn is_valid_shape(name: &str) -> bool {
    CANONICAL.contains_key(name)
}

/// The documented name `name` stands for, `rect` for none or an unknown one.
fn canonical_shape(name: Option<&str>) -> &'static str {
    let Some(name) = name else {
        return "rect";
    };
    SYNTAX
        .iter()
        .find(|(syntax, _)| *syntax == name)
        .map(|(_, shape)| *shape)
        .or_else(|| CANONICAL.get(name).copied())
        .unwrap_or("rect")
}

/// The frame a node's `form` draws around its icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconFrame {
    None,
    Circle,
    Square,
    Rounded,
}

impl IconFrame {
    pub fn from_form(form: Option<&str>) -> Self {
        match form {
            Some("circle") => Self::Circle,
            Some("square") => Self::Square,
            Some("rounded") => Self::Rounded,
            _ => Self::None,
        }
    }
}

/// What a node is drawn as, by the documented name of its shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeShape {
    Drawn(Shape),
    /// `imageSquare`
    Image,
    /// `icon`, `iconCircle`, `iconSquare` or `iconRounded`
    Icon(IconFrame),
    /// A name the drawing has no shape for; drawn as a rectangle.
    Other(&'static str),
}

impl NodeShape {
    /// The shape `name` or its alias names; the rectangle for none or an
    /// unknown name.
    pub fn named(name: Option<&str>) -> Self {
        match canonical_shape(name) {
            "imageSquare" => Self::Image,
            "icon" => Self::Icon(IconFrame::None),
            "iconCircle" => Self::Icon(IconFrame::Circle),
            "iconSquare" => Self::Icon(IconFrame::Square),
            "iconRounded" => Self::Icon(IconFrame::Rounded),
            name => Shape::from_name(name).map_or(Self::Other(name), Self::Drawn),
        }
    }

    /// The documented name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Drawn(shape) => shape.name(),
            Self::Image => "imageSquare",
            Self::Icon(IconFrame::None) => "icon",
            Self::Icon(IconFrame::Circle) => "iconCircle",
            Self::Icon(IconFrame::Square) => "iconSquare",
            Self::Icon(IconFrame::Rounded) => "iconRounded",
            Self::Other(name) => name,
        }
    }

    /// The outline drawn.
    pub fn shape(self) -> Shape {
        match self {
            Self::Drawn(shape) => shape,
            _ => Shape::Rect,
        }
    }

    /// [`Shape::span`] of a drawn shape; 0 for any other.
    pub fn span(self, size: Size, sideways: bool) -> f64 {
        match self {
            Self::Drawn(shape) => shape.span(size, sideways),
            _ => 0.0,
        }
    }
}
