//! Cluster titles, kept clear of the edges that enter a cluster at its top.
//! Pele's `svg/cluster.ts`.

use std::collections::HashMap;

use super::builder::{label_svg, num};
use crate::mermaid::{js, layout::compound::CNode, text::Label};

/// The x of every route point, by its rounded y: the edges that cross a
/// cluster's top border.
pub type Crossings = HashMap<i64, Vec<f64>>;

pub fn mark_crossings(crossings: &mut Crossings, route: &[f64]) {
    for point in route.chunks(3) {
        crossings
            .entry(js::round(point[1]) as i64)
            .or_default()
            .push(point[0]);
    }
}

/// Where a cluster title sits along the top edge so edges entering there do
/// not cross it.
pub fn cluster_title_x(c: &CNode, width: f64, crossings: &Crossings, pad: f64) -> f64 {
    if width == 0.0 {
        return c.x;
    }
    let top = c.y - c.h / 2.0;
    let left = c.x - c.w / 2.0;
    let half = width / 2.0;
    let candidates = [left + pad - 6.0 + half, c.x, left + c.w - pad + 6.0 - half];
    let Some(xs) = crossings.get(&(js::round(top) as i64)) else {
        return candidates[0];
    };
    let mut best = candidates[0];
    let mut best_gap = -1.0;
    for x in candidates {
        let mut gap = f64::INFINITY;
        for &cx in xs {
            if cx > left && cx < left + c.w {
                gap = gap.min((cx - x).abs() - half);
            }
        }
        if gap >= 6.0 {
            return x;
        }
        if gap > best_gap + 0.5 {
            best_gap = gap;
            best = x;
        }
    }
    best
}

/// Whether an edge that enters at the top runs through a title placed at
/// `x`.
pub fn title_crossed(c: &CNode, width: f64, x: f64, crossings: &Crossings) -> bool {
    if width <= 0.0 {
        return false;
    }
    let Some(xs) = crossings.get(&(js::round(c.y - c.h / 2.0) as i64)) else {
        return false;
    };
    let left = c.x - c.w / 2.0;
    xs.iter()
        .any(|&cx| cx > left && cx < left + c.w && (cx - x).abs() < width / 2.0 + 4.0)
}

/// A title an edge runs through, drawn over the edges: the text on an
/// outline in the colour of the cluster behind it, so only the line is seen
/// to stop short of the letters. `nodes` and `index` find how many clusters
/// deep the title is, each of which tints the background.
pub fn struck_title(
    label: &Label,
    x: f64,
    y: f64,
    attrs: &str,
    id: &str,
    nodes: &[CNode],
    index: usize,
) -> String {
    let p = crate::mermaid::PREFIX;
    let mut depth = 1;
    let mut parent = nodes[index].parent;
    while parent >= 0 && depth < 8 {
        depth += 1;
        parent = nodes[parent as usize].parent;
    }
    // Each cluster is half its fill over what is behind it.
    let fill = format!(
        "color-mix(in srgb,var(--_a) {}%,var(--_bg))",
        num(100.0 * (1.0 - 0.5f64.powi(depth)))
    );
    format!(
        "<g class=\"{p}-cluster-title\" data-id=\"{id}\">{}{}</g>",
        label_svg(
            label,
            x,
            y,
            &format!(
                " fill=\"none\" stroke=\"{fill}\" stroke-width=\"5\" stroke-linejoin=\"round\""
            )
        ),
        label_svg(label, x, y, attrs)
    )
}
