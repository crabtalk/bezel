//! Pele's `diagrams/treemap/layout.ts`.

use std::cmp::Ordering;

use super::model::{TreemapModel, TreemapNode};
use crate::{
    geom::{Point, Size},
    js,
};

pub struct Tile<'m> {
    pub node: &'m TreemapNode,
    pub parent: Option<usize>,
    /// Position among the top-level nodes of the branch this tile belongs to,
    /// which picks its color.
    pub series: usize,
    /// A leaf's own value, or the sum of the leaves under a section.
    pub value: f64,
    pub children: Vec<usize>,
    /// The top left corner.
    pub at: Point,
    pub size: Size,
}

pub struct Insets {
    pub gap: f64,
    /// Space a section keeps for its header, and around its children on the
    /// other three sides.
    pub header: f64,
    pub side: f64,
}

impl<'m> Tile<'m> {
    fn new(node: &'m TreemapNode, parent: Option<usize>, series: usize) -> Self {
        let value = match node.value {
            Some(value) if node.children.is_none() && value.is_finite() && value > 0.0 => value,
            _ => 0.0,
        };
        Self {
            node,
            parent,
            series,
            value,
            children: Vec::new(),
            at: Point::default(),
            size: Size::default(),
        }
    }
}

/// Lays out `shown` (indices into `tiles`) in the rectangle at `at` as one
/// squarified treemap: rows are added along the shorter side, and a row stops
/// growing when another tile would make its tiles less square.
fn squarify(tiles: &mut [Tile], shown: &[usize], total: f64, at: Point, size: Size, gap: f64) {
    let (mut x, mut y) = (at.x, at.y);
    let (mut w, mut h) = (size.w, size.h);
    let mut rest = total;
    let mut from = 0;
    while from < shown.len() {
        let scale = (w * h) / rest;
        let side = w.min(h);
        let mut sum = tiles[shown[from]].value * scale;
        let largest = sum;
        let mut worst =
            ((side * side * largest) / (sum * sum)).max((sum * sum) / (side * side * largest));
        let mut to = from + 1;
        while to < shown.len() {
            let smallest = tiles[shown[to]].value * scale;
            let next = sum + smallest;
            let ratio = ((side * side * largest) / (next * next))
                .max((next * next) / (side * side * smallest));
            if ratio > worst {
                break;
            }
            worst = ratio;
            sum = next;
            to += 1;
        }

        let thickness = sum / side;
        let mut offset = 0.0;
        for i in from..to {
            let b = &mut tiles[shown[i]];
            let length = if i == to - 1 {
                side - offset
            } else {
                (b.value * scale) / thickness
            };
            let bx = if w >= h { x } else { x + offset };
            let by = if w >= h { y + offset } else { y };
            let x0 = js::round(bx + gap / 2.0);
            let y0 = js::round(by + gap / 2.0);
            b.at = Point::new(x0, y0);
            b.size = Size::new(
                js::round(bx + (if w >= h { thickness } else { length }) - gap / 2.0) - x0,
                js::round(by + (if w >= h { length } else { thickness }) - gap / 2.0) - y0,
            );
            offset += length;
            rest -= b.value;
        }
        if w >= h {
            x += thickness;
            w -= thickness;
        } else {
            y += thickness;
            h -= thickness;
        }
        from = to;
    }
}

impl TreemapModel {
    /// Every tile, and the indices of those with a size, parents before their
    /// children.
    pub fn layout(&self, size: Size, insets: &Insets) -> (Vec<Tile<'_>>, Vec<usize>) {
        let mut all: Vec<Tile<'_>> = self
            .roots
            .iter()
            .enumerate()
            .map(|(index, node)| Tile::new(node, None, index))
            .collect();
        let top: Vec<usize> = (0..all.len()).collect();
        let mut i = 0;
        while i < all.len() {
            let node = all[i].node;
            let series = all[i].series;
            for child in node.children.iter().flatten() {
                let index = all.len();
                all.push(Tile::new(child, Some(i), series));
                all[i].children.push(index);
            }
            i += 1;
        }
        for i in (0..all.len()).rev() {
            if let Some(parent) = all[i].parent {
                let value = all[i].value;
                all[parent].value += value;
            }
        }

        let gap = insets.gap;
        let mut placed: Vec<usize> = Vec::new();
        let place = |all: &mut Vec<Tile<'_>>,
                     placed: &mut Vec<usize>,
                     children: &[usize],
                     at: Point,
                     size: Size| {
            if size.w <= 0.0 || size.h <= 0.0 {
                return;
            }
            let mut shown: Vec<usize> = children
                .iter()
                .copied()
                .filter(|&b| all[b].value > 0.0)
                .collect();
            shown.sort_by(|&a, &b| {
                (all[b].value - all[a].value)
                    .partial_cmp(&0.0)
                    .unwrap_or(Ordering::Equal)
            });
            let mut total = 0.0;
            for &b in &shown {
                total += all[b].value;
            }
            if shown.is_empty() || !total.is_finite() {
                return;
            }
            squarify(
                all,
                &shown,
                total,
                Point::new(at.x - gap / 2.0, at.y - gap / 2.0),
                Size::new(size.w + gap, size.h + gap),
                gap,
            );
            for &b in &shown {
                if all[b].size.w > 0.0 && all[b].size.h > 0.0 {
                    placed.push(b);
                }
            }
        };

        place(&mut all, &mut placed, &top, Point::default(), size);
        let mut i = 0;
        while i < placed.len() {
            let b = placed[i];
            if !all[b].children.is_empty() {
                let children = all[b].children.clone();
                let (at, size) = (all[b].at, all[b].size);
                place(
                    &mut all,
                    &mut placed,
                    &children,
                    Point::new(at.x + insets.side, at.y + insets.header),
                    Size::new(
                        size.w - 2.0 * insets.side,
                        size.h - insets.header - insets.side,
                    ),
                );
            }
            i += 1;
        }
        (all, placed)
    }
}
