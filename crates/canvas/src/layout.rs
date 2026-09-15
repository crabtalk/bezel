//! Who decides where nodes sit, and what the arrow keys walk.
//!
//! ```ignore
//! cx.new(|cx| CanvasView::new(doc, cx).with_layout(canvas::layout::DOWN))
//! ```
//!
//! [`FREE`], [`MINDMAP`], [`BALANCED`] and [`DOWN`] are answers, not the list:
//! an app's own is a [`Layout`] with its own `arrange`.

use crate::{
    mindmap::{self, Flow},
    model::{Canvas, Node},
};

/// Where each node goes, those already there left out. `held` is in a drag's
/// hand and stays.
pub type Arrange = fn(&Canvas, held: Option<&str>) -> Vec<(String, (i64, i64))>;

#[derive(Clone, Copy)]
pub struct Layout {
    /// Run after every change.
    pub arrange: Arrange,
    /// The way its trees grow, if it grows trees: arrows walk them,
    /// `backspace` takes a branch, and switching to it drops the pins. `None`
    /// moves between nearest nodes and removes one node at a time.
    pub flow: Option<Flow>,
}

/// Nodes stay where the document, the drag and the keys put them.
pub const FREE: Layout = Layout {
    arrange: |_, _| Vec::new(),
    flow: None,
};

/// Trees grow right of their roots. The default.
pub const MINDMAP: Layout = Layout {
    arrange: |canvas, held| mindmap::arrange(canvas, held, Flow::Right),
    flow: Some(Flow::Right),
};

/// A root's children split right and left.
pub const BALANCED: Layout = Layout {
    arrange: |canvas, held| mindmap::arrange(canvas, held, Flow::Both),
    flow: Some(Flow::Both),
};

/// Trees grow down from their roots.
pub const DOWN: Layout = Layout {
    arrange: |canvas, held| mindmap::arrange(canvas, held, Flow::Down),
    flow: Some(Flow::Down),
};

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
