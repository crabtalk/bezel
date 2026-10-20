//! Cluster titles, kept clear of the edges that enter a cluster at its top.
//! Pele's `svg/cluster.ts`.

use std::{collections::HashMap, fmt};

use super::builder::{fragment, num};
use crate::{
    geom::{Point, RoutePoint},
    js,
    layout::compound::CNode,
    text::Label,
};

/// The x of route points, by their rounded y: the edges that cross a
/// cluster's top border.
#[derive(Default)]
pub struct Crossings {
    xs: HashMap<i64, Vec<f64>>,
}

impl Crossings {
    pub fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, at: Point) {
        self.xs
            .entry(js::round(at.y) as i64)
            .or_default()
            .push(at.x);
    }

    /// Marks every point of `route`.
    pub fn mark(&mut self, route: &[RoutePoint]) {
        for point in route {
            self.push(point.at);
        }
    }

    /// Marks the points of `route` between its two ends.
    pub fn mark_bends(&mut self, route: &[RoutePoint]) {
        for point in route.iter().skip(1).take(route.len().saturating_sub(2)) {
            self.push(point.at);
        }
    }

    /// The x of every marked point level with the top of `c`.
    pub fn at_top(&self, c: &CNode) -> Option<&[f64]> {
        self.xs
            .get(&(js::round(c.center.y - c.size.h / 2.0) as i64))
            .map(Vec::as_slice)
    }

    /// Where a cluster title `width` wide sits along the top edge of `c` so
    /// edges entering there do not cross it.
    pub fn title_x(&self, c: &CNode, width: f64, pad: f64) -> f64 {
        if width == 0.0 {
            return c.center.x;
        }
        let left = c.center.x - c.size.w / 2.0;
        let half = width / 2.0;
        let candidates = [
            left + pad - 6.0 + half,
            c.center.x,
            left + c.size.w - pad + 6.0 - half,
        ];
        let Some(xs) = self.at_top(c) else {
            return candidates[0];
        };
        let mut best = candidates[0];
        let mut best_gap = -1.0;
        for x in candidates {
            let mut gap = f64::INFINITY;
            for &cx in xs {
                if cx > left && cx < left + c.size.w {
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

    /// Whether an edge that enters `c` at the top runs through a title
    /// `width` wide placed at `x`.
    pub fn crosses_title(&self, c: &CNode, width: f64, x: f64) -> bool {
        if width <= 0.0 {
            return false;
        }
        let Some(xs) = self.at_top(c) else {
            return false;
        };
        let left = c.center.x - c.size.w / 2.0;
        xs.iter()
            .any(|&cx| cx > left && cx < left + c.size.w && (cx - x).abs() < width / 2.0 + 4.0)
    }

    /// [`Crossings::title_x`] for a title centred when nothing crosses it:
    /// the centre, then `pad` in from the left, then from the right.
    pub fn centred_title_x(&self, c: &CNode, width: f64, pad: f64) -> f64 {
        let Some(xs) = self.at_top(c) else {
            return c.center.x;
        };
        if width == 0.0 {
            return c.center.x;
        }
        let left = c.center.x - c.size.w / 2.0;
        let half = width / 2.0;
        let mut best = c.center.x;
        let mut best_gap = -1.0;
        for x in [c.center.x, left + pad + half, left + c.size.w - pad - half] {
            let mut gap = f64::INFINITY;
            for &cx in xs {
                if cx > left && cx < left + c.size.w {
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
}

/// A title an edge runs through, drawn over the edges: the text on an
/// outline in the colour of the cluster behind it, so only the line is seen
/// to stop short of the letters. `nodes` and `index` find how many clusters
/// deep the title is, each of which tints the background.
pub fn struck_title<'a>(
    label: &'a Label,
    at: Point,
    attrs: &'a str,
    id: impl fmt::Display + 'a,
    nodes: &[CNode],
    index: usize,
) -> impl fmt::Display + 'a {
    let mut depth = 1;
    let mut parent = nodes[index].parent;
    while let Some(p) = parent
        && depth < 8
    {
        depth += 1;
        parent = nodes[p].parent;
    }
    // Each cluster is half its fill over what is behind it.
    let mix = num(100.0 * (1.0 - 0.5f64.powi(depth)));
    fragment(move |f| {
        let p = crate::PREFIX;
        write!(f, "<g class=\"{p}-cluster-title\" data-id=\"{id}\">")?;
        let outline = format!(
            " fill=\"none\" stroke=\"color-mix(in srgb,var(--_a) {mix}%,var(--_bg))\" stroke-width=\"5\" stroke-linejoin=\"round\""
        );
        write!(f, "{}{}</g>", label.svg(at, &outline), label.svg(at, attrs))
    })
}
