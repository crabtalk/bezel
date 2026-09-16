//! Who places the nodes, what the arrow keys walk, and what a tree's edits
//! mean. The consumer picks one; there is no default.
//!
//! ```ignore
//! cx.new(|cx| CanvasView::new(doc, canvas::layout::DOWN, cx))
//! ```
//!
//! [`FREE`], [`MINDMAP`], [`BALANCED`] and [`DOWN`] are answers, not the list:
//! an app's own is [`Layout::free`] or [`Layout::tree`] with a field of its
//! own.

use crate::{
    change::Change,
    drag::{self, DragHandler},
    mindmap::{self, Flow},
    model::{Canvas, Edge, Node, Side},
};

/// Where each node goes, those already there left out. `held` is in a drag's
/// hand and stays; `holds` says which nodes frame others.
pub type Arrange =
    fn(&Canvas, held: Option<&str>, holds: &dyn Fn(&Node) -> bool) -> Vec<(String, (i64, i64))>;

/// Where an arrow goes from a node.
pub type Walk = fn(&Canvas, &str, Arrow) -> Option<String>;

/// What acting on a node touches: it alone, or its branch.
pub type Reach = fn(&Canvas, &str) -> Vec<String>;

/// The connector drawn out of a node's side onto another.
pub type Link = fn(&Canvas, &str, Side, &str) -> Vec<Change>;

/// A connector let go on nothing there, with the node its kind makes.
pub type Extend = fn(&Canvas, &str, Side, (i64, i64), Node) -> Option<Vec<Change>>;

/// What a paste or a duplicate hangs under, given the primary selection.
pub type Under = fn(&Canvas, &str) -> Option<String>;

#[derive(Clone, Copy)]
pub struct Layout {
    /// Run after every change.
    pub arrange: Arrange,
    /// The way its trees grow, if it grows trees. Switching to a tree that
    /// grows another way drops the pins.
    pub flow: Option<Flow>,
    pub reach: Reach,
    pub walk: Walk,
    pub link: Link,
    pub extend: Extend,
    pub paste_under: Under,
    pub duplicate_under: Under,
    /// Whether a nudge or a drop pins what it moves.
    pub pins: bool,
    /// What dragging a node does, unless the view says otherwise.
    pub drag: DragHandler,
}

impl Layout {
    /// Nodes stay where they are put: arrows go to the nearest, acting on one
    /// touches it alone, and a connector makes a node where it is let go.
    pub const fn free(arrange: Arrange) -> Self {
        Self {
            arrange,
            flow: None,
            reach: alone,
            walk: to_nearest,
            link: plain_link,
            extend: node_there,
            paste_under: nowhere,
            duplicate_under: nowhere,
            pins: false,
            drag: drag::moves,
        }
    }

    /// Edges are branches: arrows walk them, acting on a node takes its
    /// branch, a connector hangs a child, and what is dropped stays pinned.
    pub const fn tree(arrange: Arrange, flow: Flow, walk: Walk) -> Self {
        Self {
            arrange,
            flow: Some(flow),
            reach: mindmap::branch_of,
            walk,
            link: cross_link,
            extend: hang_child,
            paste_under: under_selection,
            duplicate_under: under_parent,
            pins: true,
            drag: drag::pin,
        }
    }
}

/// Nodes stay where the document, the drag and the keys put them.
pub const FREE: Layout = Layout::free(arrange_nowhere);

/// Trees grow right of their roots.
pub const MINDMAP: Layout = Layout::tree(arrange_right, Flow::Right, walk_right);

/// A root's children split right and left.
pub const BALANCED: Layout = Layout::tree(arrange_both, Flow::Both, walk_both);

/// Trees grow down from their roots.
pub const DOWN: Layout = Layout::tree(arrange_down, Flow::Down, walk_down);

fn arrange_nowhere(
    _: &Canvas,
    _: Option<&str>,
    _: &dyn Fn(&Node) -> bool,
) -> Vec<(String, (i64, i64))> {
    Vec::new()
}

fn arrange_right(
    canvas: &Canvas,
    held: Option<&str>,
    holds: &dyn Fn(&Node) -> bool,
) -> Vec<(String, (i64, i64))> {
    mindmap::arrange(canvas, held, Flow::Right, holds)
}

fn arrange_both(
    canvas: &Canvas,
    held: Option<&str>,
    holds: &dyn Fn(&Node) -> bool,
) -> Vec<(String, (i64, i64))> {
    mindmap::arrange(canvas, held, Flow::Both, holds)
}

fn arrange_down(
    canvas: &Canvas,
    held: Option<&str>,
    holds: &dyn Fn(&Node) -> bool,
) -> Vec<(String, (i64, i64))> {
    mindmap::arrange(canvas, held, Flow::Down, holds)
}

fn walk_right(canvas: &Canvas, id: &str, arrow: Arrow) -> Option<String> {
    mindmap::walk(canvas, id, Flow::Right, arrow)
}

fn walk_both(canvas: &Canvas, id: &str, arrow: Arrow) -> Option<String> {
    mindmap::walk(canvas, id, Flow::Both, arrow)
}

fn walk_down(canvas: &Canvas, id: &str, arrow: Arrow) -> Option<String> {
    mindmap::walk(canvas, id, Flow::Down, arrow)
}

fn alone(_: &Canvas, id: &str) -> Vec<String> {
    vec![id.to_owned()]
}

fn to_nearest(canvas: &Canvas, id: &str, arrow: Arrow) -> Option<String> {
    nearest(canvas, id, arrow).map(str::to_owned)
}

fn nowhere(_: &Canvas, _: &str) -> Option<String> {
    None
}

fn under_selection(_: &Canvas, id: &str) -> Option<String> {
    Some(id.to_owned())
}

fn under_parent(canvas: &Canvas, id: &str) -> Option<String> {
    mindmap::parent(canvas, id).map(str::to_owned)
}

fn plain_link(canvas: &Canvas, from: &str, side: Side, to: &str) -> Vec<Change> {
    let edge = Edge {
        from_side: Some(side),
        ..Edge::new(canvas.mint(), from, to)
    };
    vec![Change::AddEdge { edge, index: None }]
}

/// Between two nodes of a tree, a connector is a cross link, never a branch.
fn cross_link(canvas: &Canvas, from: &str, side: Side, to: &str) -> Vec<Change> {
    let mut changes = plain_link(canvas, from, side, to);
    if let Some(Change::AddEdge { edge, .. }) = changes.first_mut() {
        edge.extra.insert(mindmap::TREE.into(), false.into());
    }
    changes
}

fn node_there(
    canvas: &Canvas,
    from: &str,
    side: Side,
    at: (i64, i64),
    mut node: Node,
) -> Option<Vec<Change>> {
    let [id, edge] = <[String; 2]>::try_from(canvas.mint_n(2)).ok()?;
    node.id = id;
    (node.x, node.y) = (at.0 - node.width / 2, at.1 - node.height / 2);
    let edge = Edge {
        from_side: Some(side),
        ..Edge::new(edge, from, node.id.as_str())
    };
    Some(vec![
        Change::AddNode { node, index: None },
        Change::AddEdge { edge, index: None },
    ])
}

fn hang_child(
    canvas: &Canvas,
    from: &str,
    _: Side,
    _: (i64, i64),
    node: Node,
) -> Option<Vec<Change>> {
    mindmap::child(canvas, from, node)
}

/// An arrow key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrow {
    Left,
    Right,
    Up,
    Down,
}

impl Arrow {
    /// One step its way.
    pub fn unit(self) -> (i64, i64) {
        match self {
            Self::Left => (-1, 0),
            Self::Right => (1, 0),
            Self::Up => (0, -1),
            Self::Down => (0, 1),
        }
    }
}

/// The nearest node from `id` toward `arrow`, within 45° of it.
pub fn nearest<'a>(canvas: &'a Canvas, id: &str, arrow: Arrow) -> Option<&'a str> {
    let center = |node: &Node| (node.x + node.width / 2, node.y + node.height / 2);
    let from = center(canvas.node(id)?);
    let (ux, uy) = arrow.unit();
    canvas
        .nodes
        .iter()
        .filter(|node| node.id != id)
        .filter_map(|node| {
            let (dx, dy) = (center(node).0 - from.0, center(node).1 - from.1);
            let ahead = dx * ux + dy * uy;
            let aside = (dx * uy - dy * ux).abs();
            (ahead > 0 && aside <= ahead).then_some((ahead + 2 * aside, node.id.as_str()))
        })
        .min()
        .map(|(_, id)| id)
}
