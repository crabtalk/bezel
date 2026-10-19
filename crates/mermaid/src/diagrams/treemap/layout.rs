//! Pele's `diagrams/treemap/layout.ts`.

use std::cmp::Ordering;

use super::model::TreemapNode;
use crate::js;

pub struct Box<'m> {
    pub node: &'m TreemapNode,
    pub parent: Option<usize>,
    /// Position among the top-level nodes of the branch this box belongs to,
    /// which picks its color.
    pub series: usize,
    /// A leaf's own value, or the sum of the leaves under a section.
    pub value: f64,
    pub children: Vec<usize>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub struct Insets {
    pub gap: f64,
    /// Space a section keeps for its header, and around its children on the
    /// other three sides.
    pub header: f64,
    pub side: f64,
}

fn make_box(node: &TreemapNode, parent: Option<usize>, series: usize) -> Box<'_> {
    let value = match node.value {
        Some(value) if node.children.is_none() && value.is_finite() && value > 0.0 => value,
        _ => 0.0,
    };
    Box {
        node,
        parent,
        series,
        value,
        children: Vec::new(),
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    }
}

/// Lays out `shown` (indices into `boxes`) in the rectangle as one squarified
/// treemap: rows are added along the shorter side, and a row stops growing
/// when another box would make its boxes less square.
#[allow(clippy::too_many_arguments)]
fn squarify(
    boxes: &mut [Box],
    shown: &[usize],
    total: f64,
    mut x: f64,
    mut y: f64,
    mut w: f64,
    mut h: f64,
    gap: f64,
) {
    let mut rest = total;
    let mut from = 0;
    while from < shown.len() {
        let scale = (w * h) / rest;
        let side = w.min(h);
        let mut sum = boxes[shown[from]].value * scale;
        let largest = sum;
        let mut worst =
            ((side * side * largest) / (sum * sum)).max((sum * sum) / (side * side * largest));
        let mut to = from + 1;
        while to < shown.len() {
            let smallest = boxes[shown[to]].value * scale;
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
            let b = &mut boxes[shown[i]];
            let length = if i == to - 1 {
                side - offset
            } else {
                (b.value * scale) / thickness
            };
            let bx = if w >= h { x } else { x + offset };
            let by = if w >= h { y + offset } else { y };
            let x0 = js::round(bx + gap / 2.0);
            let y0 = js::round(by + gap / 2.0);
            b.x = x0;
            b.y = y0;
            b.w = js::round(bx + (if w >= h { thickness } else { length }) - gap / 2.0) - x0;
            b.h = js::round(by + (if w >= h { length } else { thickness }) - gap / 2.0) - y0;
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

/// Every box, and the indices of those with a size, parents before their
/// children.
pub fn layout_treemap<'m>(
    roots: &'m [TreemapNode],
    width: f64,
    height: f64,
    insets: &Insets,
) -> (Vec<Box<'m>>, Vec<usize>) {
    let mut all: Vec<Box<'m>> = roots
        .iter()
        .enumerate()
        .map(|(index, node)| make_box(node, None, index))
        .collect();
    let top: Vec<usize> = (0..all.len()).collect();
    let mut i = 0;
    while i < all.len() {
        let node = all[i].node;
        let series = all[i].series;
        for child in node.children.iter().flatten() {
            let index = all.len();
            all.push(make_box(child, Some(i), series));
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
    let place = |all: &mut Vec<Box<'m>>,
                 placed: &mut Vec<usize>,
                 children: &[usize],
                 x: f64,
                 y: f64,
                 w: f64,
                 h: f64| {
        if w <= 0.0 || h <= 0.0 {
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
            x - gap / 2.0,
            y - gap / 2.0,
            w + gap,
            h + gap,
            gap,
        );
        for &b in &shown {
            if all[b].w > 0.0 && all[b].h > 0.0 {
                placed.push(b);
            }
        }
    };

    place(&mut all, &mut placed, &top, 0.0, 0.0, width, height);
    let mut i = 0;
    while i < placed.len() {
        let b = placed[i];
        if !all[b].children.is_empty() {
            let children = all[b].children.clone();
            let (x, y, w, h) = (all[b].x, all[b].y, all[b].w, all[b].h);
            place(
                &mut all,
                &mut placed,
                &children,
                x + insets.side,
                y + insets.header,
                w - 2.0 * insets.side,
                h - insets.header - insets.side,
            );
        }
        i += 1;
    }
    (all, placed)
}
