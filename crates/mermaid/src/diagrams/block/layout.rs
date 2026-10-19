//! Pele's `diagrams/block/layout.ts`.

use crate::{Error, Kind, js};

const MAX_SPAN: f64 = 1000.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cell {
    /// Indices of the children in the slice the cell is laid out in.
    pub children: Vec<usize>,
    /// Grid width of this container: a positive count, or -1 to put every
    /// child on one row.
    pub columns: f64,
    pub span: f64,
    pub space: bool,
    /// Height reserved above a container's grid for its title.
    pub head: f64,
    /// Size the content needs. Set by the caller for leaves, computed here
    /// for containers.
    pub nw: f64,
    pub nh: f64,
    /// Placement: grid position, then center and size in pixels.
    pub col: f64,
    pub row: f64,
    pub cols: f64,
    pub heights: Vec<f64>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Cell {
    pub fn new(nw: f64, nh: f64, span: f64) -> Self {
        Self {
            columns: -1.0,
            span,
            nw,
            nh,
            ..Self::default()
        }
    }
}

fn is_integer(x: f64) -> bool {
    x.is_finite() && x.fract() == 0.0
}

pub fn calculate_block_position(columns: f64, position: f64) -> Result<(f64, f64), Error> {
    if columns == 0.0 || !is_integer(columns) {
        return Err(Error::semantic("Columns must be an integer !== 0.").of(Kind::Block));
    }
    if position < 0.0 || !is_integer(position) {
        return Err(Error::semantic(format!(
            "Position must be a non-negative integer.{}",
            js::num_str(position)
        ))
        .of(Kind::Block));
    }
    if columns < 0.0 {
        return Ok((position, 0.0));
    }
    Ok((position % columns, (position / columns).floor()))
}

/// Puts each child in its grid cell and sizes the container. Every column is
/// as wide as the widest block needs. Every row that holds a plain block is
/// as tall as the tallest block in the container, as in Mermaid; a row of
/// nothing but containers is only as tall as they need.
fn measure(cells: &mut [Cell], at: usize, gap: f64, inset: f64) -> Result<(), Error> {
    let columns = cells[at].columns;
    let children = std::mem::take(&mut cells[at].children);
    let mut heights: Vec<f64> = Vec::new();
    let mut position = 0.0;
    let mut cols = 0.0;
    let mut cw = 0.0;
    let mut tallest = 0.0;
    for &c in &children {
        let (px, py) = calculate_block_position(columns, position)?;
        let child = &mut cells[c];
        // Mermaid lets a block wider than the rest of its row stick out of
        // the grid. Here it stops at the last column.
        let mut span = if child.span >= 1.0 {
            child.span.min(MAX_SPAN)
        } else {
            1.0
        };
        if columns > 0.0 && span > columns - px {
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
            heights.push(0.0);
        }
        if child.children.is_empty() {
            heights[row] = -1.0;
        } else if heights[row] >= 0.0 && child.nh > heights[row] {
            heights[row] = child.nh;
        }
        if child.space {
            continue;
        }
        let width = (child.nw - gap * (span - 1.0)) / span;
        if width > cw {
            cw = width;
        }
        if child.nh > tallest {
            tallest = child.nh;
        }
    }
    let mut height = (heights.len() as f64 - 1.0) * gap;
    for row in heights.iter_mut() {
        if *row < 0.0 {
            *row = tallest;
        }
        height += *row;
    }
    let cell = &mut cells[at];
    cell.children = children;
    cell.cols = cols;
    cell.heights = heights;
    cell.nw = cell.nw.max(cols * cw + (cols - 1.0) * gap + 2.0 * inset);
    cell.nh = height + 2.0 * inset + cell.head;
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
    let cw = (cell.w - 2.0 * inset - (cols - 1.0) * gap) / cols;
    let room = cell.h - 2.0 * inset - cell.head - (rows - 1.0) * gap;
    let mut needed = 0.0;
    for height in &heights {
        needed += height;
    }
    let scale = if needed > 0.0 { room / needed } else { 0.0 };
    let share = if needed > 0.0 { 0.0 } else { room / rows };
    let left = cell.x - cell.w / 2.0 + inset;
    let mut top = cell.y - cell.h / 2.0 + inset + cell.head;
    let mut row = 0;
    for c in children {
        let child = &mut cells[c];
        while (row as f64) < child.row {
            top += heights[row] * scale + share + gap;
            row += 1;
        }
        child.w = cw * child.span + gap * (child.span - 1.0);
        child.h = heights[row] * scale + share;
        child.x = left + child.col * (cw + gap) + child.w / 2.0;
        child.y = top + child.h / 2.0;
    }
}

/// Lays out the tree of cells under `cells[0]` in place. The root has no
/// border, so its grid starts at the origin.
pub fn layout(cells: &mut [Cell], gap: f64) -> Result<(f64, f64), Error> {
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
    root.w = root.nw;
    root.h = root.nh;
    root.x = root.w / 2.0;
    root.y = root.h / 2.0;
    for (i, &at) in order.iter().enumerate() {
        if !cells[at].children.is_empty() {
            arrange(cells, at, gap, if i == 0 { 0.0 } else { gap });
        }
    }
    Ok((cells[0].w, cells[0].h))
}
