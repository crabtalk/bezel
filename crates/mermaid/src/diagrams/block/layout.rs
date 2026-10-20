//! Pele's `diagrams/block/layout.ts`.

use super::types::Columns;
use crate::geom::{Point, Size};
use crate::{Error, Kind, js};

const MAX_SPAN: f64 = 1000.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cell {
    /// Indices of the children in the slice the cell is laid out in.
    pub children: Vec<usize>,
    /// Grid width of this container.
    pub columns: Columns,
    pub span: f64,
    pub space: bool,
    /// Height reserved above a container's grid for its title.
    pub head: f64,
    /// Size the content needs. Set by the caller for leaves, computed here
    /// for containers.
    pub need: Size,
    /// Placement: grid position, then center and size in pixels.
    pub col: f64,
    pub row: f64,
    pub cols: f64,
    pub heights: Vec<f64>,
    pub center: Point,
    pub size: Size,
}

fn is_integer(x: f64) -> bool {
    x.is_finite() && x.fract() == 0.0
}

impl Columns {
    /// The column and row of the grid cell at `position`.
    pub fn cell(self, position: f64) -> Result<(f64, f64), Error> {
        if let Columns::Count(columns) = self
            && (columns == 0.0 || !is_integer(columns))
        {
            return Err(Error::semantic("Columns must be an integer !== 0.").of(Kind::Block));
        }
        if position < 0.0 || !is_integer(position) {
            return Err(Error::semantic(format!(
                "Position must be a non-negative integer.{}",
                js::num_str(position)
            ))
            .of(Kind::Block));
        }
        Ok(match self {
            Columns::Auto => (position, 0.0),
            Columns::Count(columns) => (position % columns, (position / columns).floor()),
        })
    }
}

/// Puts each child in its grid cell and sizes the container. Every column is
/// as wide as the widest block needs. Every row that holds a plain block is
/// as tall as the tallest block in the container, as in Mermaid; a row of
/// nothing but containers is only as tall as they need.
fn measure(cells: &mut [Cell], at: usize, gap: f64, inset: f64) -> Result<(), Error> {
    let columns = cells[at].columns;
    let children = std::mem::take(&mut cells[at].children);
    // `None` for a row that holds a plain block.
    let mut heights: Vec<Option<f64>> = Vec::new();
    let mut position = 0.0;
    let mut cols = 0.0;
    let mut cw = 0.0;
    let mut tallest = 0.0;
    for &c in &children {
        let (px, py) = columns.cell(position)?;
        let child = &mut cells[c];
        // Mermaid lets a block wider than the rest of its row stick out of
        // the grid. Here it stops at the last column.
        let mut span = if child.span >= 1.0 {
            child.span.min(MAX_SPAN)
        } else {
            1.0
        };
        if let Columns::Count(columns) = columns
            && span > columns - px
        {
            span = columns - px;
        }
        child.span = span;
        child.col = px;
        child.row = py;
        position += span;
        if px + span > cols {
            cols = px + span;
        }
        let row = py as usize;
        if row == heights.len() {
            heights.push(Some(0.0));
        }
        if child.children.is_empty() {
            heights[row] = None;
        } else if let Some(height) = heights[row]
            && child.need.h > height
        {
            heights[row] = Some(child.need.h);
        }
        if child.space {
            continue;
        }
        let width = (child.need.w - gap * (span - 1.0)) / span;
        if width > cw {
            cw = width;
        }
        if child.need.h > tallest {
            tallest = child.need.h;
        }
    }
    let heights: Vec<f64> = heights
        .into_iter()
        .map(|row| row.unwrap_or(tallest))
        .collect();
    let mut height = (heights.len() as f64 - 1.0) * gap;
    for row in &heights {
        height += row;
    }
    let cell = &mut cells[at];
    cell.children = children;
    cell.cols = cols;
    cell.heights = heights;
    cell.need.w = cell
        .need
        .w
        .max(cols * cw + (cols - 1.0) * gap + 2.0 * inset);
    cell.need.h = height + 2.0 * inset + cell.head;
    Ok(())
}

/// Gives each child its cell, sharing out any room the container has beyond
/// what it needs.
fn arrange(cells: &mut [Cell], at: usize, gap: f64, inset: f64) {
    let cell = &cells[at];
    let cols = cell.cols;
    let heights = cell.heights.clone();
    let children = cell.children.clone();
    let rows = heights.len() as f64;
    let cw = (cell.size.w - 2.0 * inset - (cols - 1.0) * gap) / cols;
    let room = cell.size.h - 2.0 * inset - cell.head - (rows - 1.0) * gap;
    let mut needed = 0.0;
    for height in &heights {
        needed += height;
    }
    let scale = if needed > 0.0 { room / needed } else { 0.0 };
    let share = if needed > 0.0 { 0.0 } else { room / rows };
    let left = cell.center.x - cell.size.w / 2.0 + inset;
    let mut top = cell.center.y - cell.size.h / 2.0 + inset + cell.head;
    let mut row = 0;
    for c in children {
        let child = &mut cells[c];
        while (row as f64) < child.row {
            top += heights[row] * scale + share + gap;
            row += 1;
        }
        child.size = Size::new(
            cw * child.span + gap * (child.span - 1.0),
            heights[row] * scale + share,
        );
        child.center = Point::new(
            left + child.col * (cw + gap) + child.size.w / 2.0,
            top + child.size.h / 2.0,
        );
    }
}

/// Lays out the tree of cells under `cells[0]` in place and gives its size.
/// The root has no border, so its grid starts at the origin.
pub fn layout(cells: &mut [Cell], gap: f64) -> Result<Size, Error> {
    let mut order = vec![0];
    let mut i = 0;
    while i < order.len() {
        order.extend(cells[order[i]].children.iter().copied());
        i += 1;
    }
    for i in (0..order.len()).rev() {
        let at = order[i];
        if !cells[at].children.is_empty() {
            measure(cells, at, gap, if i == 0 { 0.0 } else { gap })?;
        }
    }
    let root = &mut cells[0];
    root.size = root.need;
    root.center = Point::new(root.size.w / 2.0, root.size.h / 2.0);
    for (i, &at) in order.iter().enumerate() {
        if !cells[at].children.is_empty() {
            arrange(cells, at, gap, if i == 0 { 0.0 } else { gap });
        }
    }
    Ok(cells[0].size)
}
