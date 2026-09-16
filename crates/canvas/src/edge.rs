//! What an edge is: where it runs, how thick it paints, how near a press must
//! come to pick it, and what its label edits — keyed by the edge's `type`, as
//! a node's kind is by its own.
//!
//! ```ignore
//! CanvasView::new(doc, layout, cx).with_edge_kinds(EdgeKinds::new().with("straight", edge::line()))
//! ```
//!
//! An edge kind holds no gpui type: the view paints the [`Path`] it answers.

use std::{collections::HashMap, rc::Rc};

use crate::{
    handle::{self, Handle, Which},
    kind::{self, Capabilities, Capability},
    model::Edge,
    path::{self, Ends, Path},
};

/// Where an edge runs.
pub type Draw = fn(&Ends) -> Path;
pub type ReadEdge = Rc<dyn Fn(&Edge) -> String>;
pub type WriteEdge = Rc<dyn Fn(&mut Edge, String)>;

/// The string field editing an edge in place changes.
#[derive(Clone)]
pub struct EdgeField {
    pub read: ReadEdge,
    pub write: WriteEdge,
}

impl EdgeField {
    pub fn new(
        read: impl Fn(&Edge) -> String + 'static,
        write: impl Fn(&mut Edge, String) + 'static,
    ) -> Self {
        Self {
            read: Rc::new(read),
            write: Rc::new(write),
        }
    }

    /// The spec's `label`, which an empty string takes away.
    pub fn label() -> Self {
        Self::new(
            |edge| edge.label.clone().unwrap_or_default(),
            |edge, label| edge.label = (!label.is_empty()).then_some(label),
        )
    }
}

/// What the canvas reads of an edge kind without a window.
#[derive(Clone)]
pub struct EdgeRules {
    /// What an edge of this kind lets the reader do.
    pub can: Capabilities,
    /// What `f2` and a double-click edit.
    pub edit: Option<EdgeField>,
    /// What a picked edge paints, and what dragging each one does.
    pub handles: fn(&Edge) -> Vec<Handle>,
}

#[derive(Clone)]
pub struct EdgeKind {
    pub rules: EdgeRules,
    pub path: Draw,
    /// How near a press must come to pick it, in screen pixels.
    pub reach: f32,
    /// How thick it paints, in canvas units.
    pub weight: f32,
}

impl EdgeKind {
    /// Painted by `path`, editing its label, picked within six pixels.
    pub fn new(path: Draw) -> Self {
        Self {
            rules: EdgeRules {
                can: Capabilities::ALL,
                edit: Some(EdgeField::label()),
                handles: handle::both_ends,
            },
            path,
            reach: 6.0,
            weight: 1.0,
        }
    }

    /// What an edge of this kind lets the reader do. An edge's own field still
    /// refuses what the kind allows.
    pub fn can(mut self, can: Capabilities) -> Self {
        self.rules.can = can;
        self
    }

    pub fn edit(mut self, field: Option<EdgeField>) -> Self {
        self.rules.edit = field;
        self
    }

    /// What an edge of this kind paints when it is picked.
    /// [`handle::bare_edge`] paints none.
    pub fn handles(mut self, handles: fn(&Edge) -> Vec<Handle>) -> Self {
        self.rules.handles = handles;
        self
    }

    pub fn reach(mut self, reach: f32) -> Self {
        self.reach = reach;
        self
    }

    pub fn weight(mut self, weight: f32) -> Self {
        self.weight = weight;
        self
    }
}

/// Our own edge fields: the handle each end left from, when it was not the
/// middle of a side.
pub const FROM_HANDLE: &str = "fromHandle";
pub const TO_HANDLE: &str = "toHandle";

/// The handle an end names, if it names one.
pub fn handle_of(edge: &Edge, which: Which) -> Option<&str> {
    let field = match which {
        Which::From => FROM_HANDLE,
        Which::To => TO_HANDLE,
    };
    edge.extra.get(field).and_then(|name| name.as_str())
}

/// The spec's edge: two quadratic halves bending out of the sides they leave.
pub fn curve() -> EdgeKind {
    EdgeKind::new(path::curve)
}

/// A straight line between the sides it leaves.
pub fn line() -> EdgeKind {
    EdgeKind::new(path::line)
}

/// Every edge kind a canvas paints, by `type`.
#[derive(Clone)]
pub struct EdgeKinds {
    kinds: HashMap<String, EdgeKind>,
    /// What an edge naming no type, or one nothing names, is.
    plain: EdgeKind,
}

/// What an edge naming no type is.
pub const CURVE: &str = "curve";

impl EdgeKinds {
    pub fn new() -> Self {
        Self {
            kinds: HashMap::from([(CURVE.to_owned(), curve())]),
            plain: curve(),
        }
    }

    /// Add a kind, or replace one — `curve` included.
    pub fn with(mut self, name: impl Into<String>, kind: EdgeKind) -> Self {
        self.kinds.insert(name.into(), kind);
        self
    }

    pub fn get(&self, edge: &Edge) -> &EdgeKind {
        match &edge.kind {
            Some(name) => self.kinds.get(name).unwrap_or(&self.plain),
            None => &self.plain,
        }
    }

    /// Whether `edge` lets the reader do `what`: its kind's rules, and its own
    /// field where it names one.
    pub fn allows(&self, edge: &Edge, what: Capability) -> bool {
        self.get(edge).rules.can.has(what) && kind::allows(&edge.extra, what)
    }
}

impl Default for EdgeKinds {
    fn default() -> Self {
        Self::new()
    }
}
