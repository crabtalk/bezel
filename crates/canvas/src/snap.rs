//! Where a dragged box settles: onto a line another node shares, else onto the
//! grid.

use crate::model::{Canvas, Node};

/// How moving boxes settle. Neither, unless an app says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Snap {
    /// Positions and sizes land on multiples of it, and dots mark it.
    pub grid: Option<i64>,
    /// Edges and middles catch on other nodes' within reach, drawing a guide.
    pub guides: bool,
}

/// Which way a guide runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// A vertical line at `x = at`.
    X,
    /// A horizontal line at `y = at`.
    Y,
}

/// A line two boxes share, from `from` to `to` along it, in canvas units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Guide {
    pub axis: Axis,
    pub at: i64,
    pub from: i64,
    pub to: i64,
}

/// Where a box of `size` carried to `to` settles, and the guides that caught
/// it. The nodes in `moving` are not lines to catch on.
pub fn settle(
    canvas: &Canvas,
    moving: &[String],
    to: (i64, i64),
    size: (i64, i64),
    snap: Snap,
    reach: i64,
) -> ((i64, i64), Vec<Guide>) {
    let others: Vec<&Node> = match snap.guides {
        true => canvas
            .nodes
            .iter()
            .filter(|node| !moving.contains(&node.id))
            .collect(),
        false => Vec::new(),
    };
    let mut at = to;
    let mut guides = Vec::new();
    for axis in [Axis::X, Axis::Y] {
        let (start, length) = along(axis, to, size);
        // The smallest shift onto another node's line: shift, line, node.
        let mut best: Option<(i64, i64, &Node)> = None;
        for node in &others {
            let (their_start, their_length) =
                along(axis, (node.x, node.y), (node.width, node.height));
            for mine in lines(start, length) {
                for theirs in lines(their_start, their_length) {
                    let shift = theirs - mine;
                    if shift.abs() <= reach && best.is_none_or(|(b, _, _)| shift.abs() < b.abs()) {
                        best = Some((shift, theirs, node));
                    }
                }
            }
        }
        let settled = match (best, snap.grid) {
            (Some((shift, line, node)), _) => {
                let across = match axis {
                    Axis::X => Axis::Y,
                    Axis::Y => Axis::X,
                };
                let (their, their_length) =
                    along(across, (node.x, node.y), (node.width, node.height));
                let (mine, my_length) = along(across, to, size);
                guides.push(Guide {
                    axis,
                    at: line,
                    from: their.min(mine),
                    to: (their + their_length).max(mine + my_length),
                });
                start + shift
            }
            (None, Some(grid)) => round_to(start, grid),
            (None, None) => start,
        };
        match axis {
            Axis::X => at.0 = settled,
            Axis::Y => at.1 = settled,
        }
    }
    (at, guides)
}

/// `value` at the nearest multiple of `step`.
pub fn round_to(value: i64, step: i64) -> i64 {
    if step <= 0 {
        return value;
    }
    (value as f64 / step as f64).round() as i64 * step
}

/// A box's start and length along `axis`.
fn along(axis: Axis, at: (i64, i64), size: (i64, i64)) -> (i64, i64) {
    match axis {
        Axis::X => (at.0, size.0),
        Axis::Y => (at.1, size.1),
    }
}

/// A box's start, middle and end along one axis.
fn lines(start: i64, length: i64) -> [i64; 3] {
    [start, start + length / 2, start + length]
}
