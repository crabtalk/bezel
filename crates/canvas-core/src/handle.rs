//! The handles a kind declares: where each sits, and what dragging it does.
//!
//! A node's sit on its sides; an edge's sit along its path. The canvas works
//! out where one lands from its [`Spot`], so a press finds it without waiting
//! for a frame to be laid out, and an edge names the one it left from.

use gpui::{Point, point};

use crate::{
    model::{Edge, Node, Side},
    path::{Anchor, Path, Rect},
};

/// Which end of an edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    From,
    To,
}

/// Where a handle sits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spot {
    /// On a node's side, `at` along it: 0 at its top or left corner, 1 at the
    /// other, 0.5 in the middle.
    Side { side: Side, at: f32 },
    /// A node's bottom right corner.
    Corner,
    /// Where an edge leaves or arrives.
    End(Which),
    /// Along an edge's path, 0 at its start and 1 at its end.
    Along(f32),
}

impl Spot {
    /// Where it sits on a node's box, and the way out of the box there.
    /// `None` when it is an edge's.
    pub fn on(self, rect: Rect) -> Option<Anchor> {
        let Rect { x, y, w, h } = rect;
        Some(match self {
            Self::Side { side, at } => match side {
                Side::Top => (point(x + w * at, y), point(0.0, -1.0)),
                Side::Right => (point(x + w, y + h * at), point(1.0, 0.0)),
                Side::Bottom => (point(x + w * at, y + h), point(0.0, 1.0)),
                Side::Left => (point(x, y + h * at), point(-1.0, 0.0)),
            },
            Self::Corner => (point(x + w, y + h), point(0.0, 0.0)),
            Self::End(_) | Self::Along(_) => return None,
        })
    }

    /// Where it sits along an edge's path. `None` when it is a node's.
    pub fn along(self, path: &Path) -> Option<Point<f32>> {
        match self {
            Self::End(Which::From) => Some(path.start()),
            Self::End(Which::To) => Some(path.end()),
            Self::Along(t) => Some(path.at(t)),
            Self::Side { .. } | Self::Corner => None,
        }
    }
}

/// What dragging a handle does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Draw a connector out of the node.
    Connect,
    /// Pull the node's box.
    Resize,
    /// Carry an edge's end onto another node.
    Reconnect(Which),
}

/// A handle a kind paints and a press can take.
#[derive(Clone, Debug, PartialEq)]
pub struct Handle {
    /// Its own name, which an edge writes down to say where it left from.
    pub id: String,
    pub spot: Spot,
    pub role: Role,
}

impl Handle {
    pub fn new(id: impl Into<String>, spot: Spot, role: Role) -> Self {
        Self {
            id: id.into(),
            spot,
            role,
        }
    }

    /// A connector drawn out of the middle of a side.
    pub fn connect(side: Side) -> Self {
        Self::new(name(side), Spot::Side { side, at: 0.5 }, Role::Connect)
    }

    /// The corner that pulls the box.
    pub fn corner() -> Self {
        Self::new("corner", Spot::Corner, Role::Resize)
    }

    /// An edge's end, to carry onto another node.
    pub fn end(which: Which) -> Self {
        let id = match which {
            Which::From => "from",
            Which::To => "to",
        };
        Self::new(id, Spot::End(which), Role::Reconnect(which))
    }

    /// The side it leaves from, for an edge drawn out of it.
    pub fn side(&self) -> Option<Side> {
        match self.spot {
            Spot::Side { side, .. } => Some(side),
            _ => None,
        }
    }
}

/// What the spec's node kinds declare: a connector on each side, and the
/// corner.
pub fn sides_and_corner(_: &Node) -> Vec<Handle> {
    [Side::Top, Side::Right, Side::Bottom, Side::Left]
        .map(Handle::connect)
        .into_iter()
        .chain([Handle::corner()])
        .collect()
}

/// What the spec's edge declares: both ends, to carry onto another node.
pub fn both_ends(_: &Edge) -> Vec<Handle> {
    vec![Handle::end(Which::From), Handle::end(Which::To)]
}

/// A node kind that paints no handle.
pub fn bare_node(_: &Node) -> Vec<Handle> {
    Vec::new()
}

/// An edge kind that paints no handle.
pub fn bare_edge(_: &Edge) -> Vec<Handle> {
    Vec::new()
}

/// A side's own name, which is what a handle on it is called.
pub fn name(side: Side) -> &'static str {
    match side {
        Side::Top => "top",
        Side::Right => "right",
        Side::Bottom => "bottom",
        Side::Left => "left",
    }
}
